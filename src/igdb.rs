use std::{collections::HashMap, num::NonZeroU32};

use anyhow::Context;
use async_trait::async_trait;
use governor::{
    clock::DefaultClock,
    middleware::NoOpMiddleware,
    state::{InMemoryState, NotKeyed},
    Quota, RateLimiter,
};
use hyper::{client::HttpConnector, Body, Client, Method, Request, Uri};
use hyper_tls::HttpsConnector;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sqlx::{Connection, Transaction};
use time::OffsetDateTime;

trait HasCacheId {
    fn id(&self) -> u32;
}

// can't be arsed to type all these bounds all the time
trait Cacheable: Send + Sync + Serialize + DeserializeOwned + HasCacheId {}
impl<T> Cacheable for T where T: Send + Sync + Serialize + DeserializeOwned + HasCacheId {}

#[derive(Debug, Deserialize, Serialize)]
pub struct Game {
    pub id: u32,
    pub name: String,
    pub slug: String,
    #[serde(with = "time::serde::timestamp::option", default)]
    pub first_release_date: Option<OffsetDateTime>,
    #[serde(default)]
    pub genres: Vec<u32>,
    pub summary: Option<String>,
    pub url: String,
    #[serde(rename = "cover")]
    pub cover_id: u32,
}

impl HasCacheId for Game {
    fn id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Genre {
    pub id: u32,
    pub name: String,
}

impl HasCacheId for Genre {
    fn id(&self) -> u32 {
        self.id
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct Cover {
    pub id: u32,
    pub url: String,
}

impl HasCacheId for Cover {
    fn id(&self) -> u32 {
        self.id
    }
}

#[async_trait]
pub trait IGDBCache: Sync {
    #[allow(unused_variables)]
    async fn set<T>(&self, id: u32, endpoint: &str, val: T) -> anyhow::Result<()>
    where
        T: Send + Serialize,
    {
        Ok(())
    }

    #[allow(unused_variables)]
    async fn set_many<T>(&self, endpoint: &str, vals: Vec<(u32, T)>) -> anyhow::Result<()>
    where
        T: Send + Serialize,
    {
        Ok(())
    }

    #[allow(unused_variables)]
    async fn get<T>(&self, id: u32, endpoint: &str) -> anyhow::Result<Option<T>>
    where
        T: Send + DeserializeOwned,
    {
        Ok(None)
    }

    #[allow(unused_variables)]
    async fn get_many<T>(&self, endpoint: &str, ids: &[u32]) -> anyhow::Result<HashMap<u32, T>>
    where
        T: Send + DeserializeOwned,
    {
        Ok(HashMap::with_capacity(0))
    }
}

// for testing or debugging the api
#[allow(dead_code)]
pub struct NoOpCache {}

#[async_trait]
impl IGDBCache for NoOpCache {}

pub struct SqliteCache {
    path: String,
}

impl SqliteCache {
    pub fn new(path: String) -> Self {
        Self { path }
    }

    async fn get_conn(&self) -> anyhow::Result<sqlx::SqliteConnection> {
        let conn = sqlx::SqliteConnection::connect(&self.path).await?;
        Ok(conn)
    }

    async fn _set<'a, T>(
        &self,
        tx: &mut Transaction<'a, sqlx::Sqlite>,
        id: u32,
        endpoint: &str,
        val: T,
    ) -> anyhow::Result<()>
    where
        T: Send + Serialize,
    {
        let val = serde_json::to_string(&val)?;
        sqlx::query("INSERT INTO igdb_cache (igdb_id, endpoint, value) VALUES (?,?,?)")
            .bind(id)
            .bind(endpoint)
            .bind(val)
            .execute(tx)
            .await?;
        log::debug!("set cache for ({endpoint}, {id})");
        Ok(())
    }
}

#[async_trait]
impl IGDBCache for SqliteCache {
    async fn set<T>(&self, id: u32, endpoint: &str, val: T) -> anyhow::Result<()>
    where
        T: Send + Serialize,
    {
        let mut conn = self.get_conn().await?;
        let mut tx = conn.begin().await?;
        self._set(&mut tx, id, endpoint, val).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn set_many<T>(&self, endpoint: &str, vals: Vec<(u32, T)>) -> anyhow::Result<()>
    where
        T: Send + Serialize,
    {
        log::debug!("set {} objects for endpoint {}", vals.len(), endpoint);
        let mut conn = self.get_conn().await?;
        let mut tx = conn.begin().await?;

        for (id, val) in vals {
            self._set(&mut tx, id, endpoint, val).await?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn get<T>(&self, id: u32, endpoint: &str) -> anyhow::Result<Option<T>>
    where
        T: DeserializeOwned,
    {
        let mut conn = self.get_conn().await?;
        let raw_val = sqlx::query_as::<_, (String,)>(
            "SELECT value FROM igdb_cache WHERE igdb_id = ? AND endpoint = ?",
        )
        .bind(id)
        .bind(endpoint)
        .fetch_optional(&mut conn)
        .await?;

        let val = raw_val.map(|s| serde_json::from_str(&s.0)).transpose()?;
        if val.is_none() {
            log::debug!("cache miss for ({endpoint},{id})");
        }
        Ok(val)
    }

    async fn get_many<T>(&self, endpoint: &str, ids: &[u32]) -> anyhow::Result<HashMap<u32, T>>
    where
        T: Send + DeserializeOwned,
    {
        let mut result = HashMap::new();
        for id in ids {
            if let Some(val) = self.get(*id, endpoint).await? {
                result.insert(*id, val);
            }
        }

        Ok(result)
    }
}

pub struct IGDB<Cache> {
    client: hyper::Client<HttpsConnector<HttpConnector>, Body>,
    client_id: String,

    // ignore the expiration
    access_token: String,
    limiter: RateLimiter<NotKeyed, InMemoryState, DefaultClock, NoOpMiddleware>,
    cache: Cache,
}

impl<Cache> IGDB<Cache>
where
    Cache: IGDBCache,
{
    pub async fn new(cache: Cache) -> anyhow::Result<Self> {
        let client_id = std::env::var("IGDB_TWITCH_CLIENT_ID")
            .context("env var IGDB_TWITCH_CLIENT_ID not found")?;
        let client_secret = std::env::var("IGDB_TWITCH_CLIENT_SECRET")
            .context("env var IGDB_TWITCH_CLIENT_SECRET not found")?;

        #[derive(Deserialize)]
        struct TwitchToken {
            access_token: String,
        }

        let client = Client::builder().build(hyper_tls::HttpsConnector::new());

        let access_token = match std::env::var("TWITCH_ACCESS_TOKEN") {
            Ok(tok) => {
                log::info!("Found an access token in environment");
                tok
            }
            Err(_) => {
                let twitch_uri = Uri::builder()
                    .scheme("https")
                    .authority("id.twitch.tv")
                    .path_and_query(format!(
                        "/oauth2/token?client_id={}&client_secret={}&grant_type=client_credentials",
                        client_id, client_secret
                    ))
                    .build()?;

                let req = Request::builder()
                    .uri(twitch_uri)
                    .method(Method::POST)
                    .body(Body::empty())?;

                let mut resp = client.request(req).await?;
                let body = hyper::body::to_bytes(resp.body_mut())
                    .await
                    .context("cannot get body for twitch token")?;
                let strbody = std::str::from_utf8(&body).context("invalid utf-8 received")?;
                let twitch_resp: TwitchToken = serde_json::from_str(strbody)
                    .with_context(|| format!("invalid response from twitch: {:?}", strbody))?;
                twitch_resp.access_token
            }
        };

        let limiter = RateLimiter::direct(Quota::per_second(NonZeroU32::new(4).unwrap()));

        Ok(Self {
            client,
            client_id,
            access_token,
            limiter,
            cache,
        })
    }

    async fn req_igdb<T>(&self, endpoint: &str, body: String) -> anyhow::Result<T>
    where
        T: DeserializeOwned,
    {
        let req = Request::builder()
            .method(Method::POST)
            .uri(format!("https://api.igdb.com/v4/{endpoint}"))
            .header("Client-ID", &self.client_id)
            .header("Authorization", format!("Bearer {}", self.access_token))
            .header("Accept", "application/json")
            .body(Body::from(body.clone()))?;
        self.limiter.until_ready().await;
        let mut resp = self.client.request(req).await?;

        let resp_body = hyper::body::to_bytes(resp.body_mut()).await?;
        let strbody = std::str::from_utf8(&resp_body).context("invalid utf-8 received")?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "invalid request for endpoint {endpoint} with body {body}: {strbody}"
            ));
        }

        match serde_json::from_str(strbody) {
            Ok(results) => Ok(results),
            Err(err) => {
                log::error!("Invalid json when fetching {endpoint} with body {body}. Got response: {strbody}\n{err:?}");
                Err(err.into())
            }
        }
    }

    async fn get_objects<T>(
        &self,
        endpoint: &str,
        fields: &str,
        ids: &[u32],
    ) -> anyhow::Result<Vec<T>>
    where
        T: Cacheable,
    {
        let cached_items = self.cache.get_many::<T>(endpoint, &ids[..]).await?;

        let ids_str = ids
            .iter()
            .filter(|i| !cached_items.contains_key(*i))
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let mut fetched_items = if ids_str.is_empty() {
            Vec::new()
        } else {
            // maximum limit is 500 and I don't have anything bigger than that, so
            // avoid doing any pagination at all
            let body = format!("limit 500; fields {fields}; where id=({});", ids_str);
            let result: Vec<T> = self.req_igdb(endpoint, body).await?;
            self.cache
                .set_many(endpoint, result.iter().map(|g| (g.id(), g)).collect())
                .await?;
            if result.len() != ids.len() {
                log::warn!("PAGINATION!!!! {} vs {}", result.len(), ids.len());
            }
            result
        };

        fetched_items.reserve(cached_items.len());
        for (_, cg) in cached_items {
            fetched_items.push(cg);
        }

        Ok(fetched_items)
    }

    pub async fn get_games(&self, ids: &[u32]) -> anyhow::Result<Vec<Game>> {
        self.get_objects("games", "*", ids).await
    }

    pub async fn get_genres(&self, ids: &[u32]) -> anyhow::Result<Vec<Genre>> {
        self.get_objects("genres", "*", ids).await
    }

    pub async fn get_covers(&self, cover_ids: &[u32]) -> anyhow::Result<Vec<Cover>> {
        let mut covers: Vec<Cover> = self.get_objects("covers", "*", cover_ids).await?;
        covers.iter_mut().for_each(|cover| {
            cover.url = format!("https:{}", cover.url.replace("t_thumb", "t_cover_med"));
        });
        Ok(covers)
    }
}
