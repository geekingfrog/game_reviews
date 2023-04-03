use std::collections::{BTreeMap, BTreeSet};

use askama::Template;
use sqlx::{Executor, Connection};

use game_reviews::igdb::{self, IGDBCache};

#[derive(Debug)]
struct Game {
    title: String,
    link: String,
    date_released: Option<String>,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    heart_count: Option<i64>,
    genres: Vec<String>,
}

#[derive(sqlx::FromRow, Debug)]
struct Category {
    id: i64,
    title: String,
    #[allow(dead_code)]
    sort_order: i64,
    description: String,
}

#[derive(sqlx::FromRow, Debug)]
struct GameReview {
    #[allow(dead_code)]
    id: i64,
    igdb_id: u32,
    title: String,
    year_played: Option<String>,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    heart_count: Option<i64>,
    #[allow(dead_code)]
    category_id: i64,
}

struct Section {
    category: Category,
    games: Vec<Game>,
}

#[derive(Template)]
#[template(path = "reviews.html")]
struct ReviewTemplate {
    sections: Vec<Section>,
}

mod filters {
    /// for the heart count
    pub fn repeat<T: std::fmt::Display>(s: T, count: &&i64) -> askama::Result<String> {
        let count = (**count).try_into().unwrap();
        Ok(s.to_string().repeat(count))
    }
}

async fn get_sections<Cache: IGDBCache>(
    sqlite_path: &str,
    igdb: &igdb::IGDB<Cache>,
) -> anyhow::Result<Vec<Section>> {
    let mut conn = sqlx::SqliteConnection::connect(sqlite_path).await?;

    let categories = sqlx::query_as::<_, Category>("SELECT * from category ORDER BY sort_order")
        .fetch_all(&mut conn)
        .await?;
    let mut sections = Vec::with_capacity(categories.len());

    for cat in categories {
        let games = sqlx::query_as::<_, GameReview>(
            "SELECT *
            FROM game_review
            WHERE category_id = ?
            GROUP BY id
            ORDER BY rating DESC, title",
        )
        .bind(cat.id)
        .fetch_all(&mut conn)
        .await?;

        // log::debug!("games: {games:#?}");

        let game_ids = games.iter().map(|g| g.igdb_id).collect::<Vec<_>>();
        let igdb_games = igdb.get_games(&game_ids[..]).await?;

        let genre_ids = igdb_games.iter().fold(BTreeSet::new(), |mut genres, g| {
            for genre in &g.genres {
                genres.insert(*genre);
            }
            genres
        });

        let genre_ids = genre_ids.into_iter().collect::<Vec<_>>();
        let genres = igdb.get_genres(&genre_ids[..]).await?;

        let cover_ids = igdb_games.iter().map(|g| g.cover_id).collect::<Vec<_>>();
        // log::debug!("searching for covers: {cover_ids:?}");
        let covers = igdb.get_covers(&cover_ids[..]).await?;
        // log::debug!("covers: {:#?}", covers);

        sections.push(Section {
            category: cat,
            games: todo!(),
        });
    }

    todo!()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // let cache = igdb::NoOpCache {};
    let sqlite_path = "game_reviews.sqlite3";
    let cache = igdb::SqliteCache::new(sqlite_path.to_string());
    let igdb = igdb::IGDB::new(cache).await?;
    get_sections(sqlite_path, &igdb).await?;

    // let mut conn = sqlx::SqliteConnection::connect(sqlite_path).await?;
    // let stuff = sqlx::query_as::<_, (String,)>(
    //     "select value from igdb_cache where igdb_id = ? and endpoint = ?"
    // )
    //     .bind(99471)
    //     .bind("covers")
    //     .fetch_one(&mut conn).await?;

    // println!("{stuff:#?}");
    // println!("{:#?}", igdb.get_covers(&[99471]).await?);


    // let games = sqlx::query_as::<_, Game>(
    //     "SELECT game.*, GROUP_CONCAT(tag.value, '/') as tags
    //     FROM game
    //     LEFT OUTER JOIN game_tag ON game_tag.game_id = game.id
    //     LEFT OUTER JOIN tag ON game_tag.tag_id = tag.id
    //     GROUP BY game.id
    //     ORDER BY game.rating DESC, game.title",
    // )
    // .fetch_all(&mut conn)
    // .await?;

    // let mut conn = sqlx::SqliteConnection::connect("game_reviews.sqlite3").await?;
    // let mut wrt = std::io::BufWriter::new(std::io::stdout());

    // let categories = sqlx::query_as::<_, Category>("SELECT * from category ORDER BY sort_order")
    //     .fetch_all(&mut conn)
    //     .await?;

    // let mut sections = Vec::with_capacity(categories.len());

    // for cat in categories {
    //     let games = sqlx::query_as::<_, Game>(
    //         "SELECT game.*, GROUP_CONCAT(tag.value, '/') as tags
    //         FROM game
    //         LEFT OUTER JOIN game_tag ON game_tag.game_id = game.id
    //         LEFT OUTER JOIN tag ON game_tag.tag_id = tag.id
    //         WHERE game.category_id = ?
    //         GROUP BY game.id
    //         ORDER BY game.rating DESC, game.title",
    //     )
    //     .bind(cat.id)
    //     .fetch_all(&mut conn)
    //     .await?;

    //     sections.push(Section {
    //         category: cat,
    //         games,
    //     });
    // }

    // let reviews = ReviewTemplate { sections };
    // reviews.write_into(&mut wrt)?;

    Ok(())
}

#[cfg(test)]
mod test {
    #[test]
    fn test_stuff() {
        #[derive(Debug, serde::Deserialize, PartialEq, Eq)]
        struct Foo {
            #[serde(with = "time::serde::timestamp")]
            date: time::OffsetDateTime,
        }
        let raw = r#"{"date": 1518393600}"#;
        let parsed: Foo = serde_json::from_str(raw).unwrap();
        assert_eq!(
            parsed,
            Foo {
                date: time::macros::datetime!(2020-01-02 03:04:05 UTC)
            }
        );
    }
}
