use std::collections::BTreeSet;

use askama::Template;
use sqlx::Connection;

use game_reviews::igdb::{self, IGDBCache};

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
    reviews: Vec<Review>,
}

#[derive(Debug)]
struct Review {
    title: String,
    link: String,
    cover_url: String,
    date_released: Option<String>,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    heart_count: Option<i64>,
    genres: Vec<String>,
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
        let game_reviews = sqlx::query_as::<_, GameReview>(
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

        let game_ids = game_reviews.iter().map(|g| g.igdb_id).collect::<Vec<_>>();
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

        let games = game_reviews
            .iter()
            .map(|gr| make_review(&genres[..], &covers[..], &igdb_games[..], gr))
            .collect();
        sections.push(Section {
            category: cat,
            reviews: games,
        });
    }

    Ok(sections)
}

fn make_review(
    genres: &[igdb::Genre],
    covers: &[igdb::Cover],
    games: &[igdb::Game],
    gr: &GameReview,
) -> Review {
    let game = games
        .iter()
        .find(|g| g.id == gr.igdb_id)
        .expect(&format!("can't find igdb game for {gr:?}"));

    let cover = covers
        .iter()
        .find(|c| c.id == game.cover_id)
        .expect(&format!("can't find cover for igdb game {game:?}"));

    let genres = genres
        .iter()
        .filter_map(|genre| {
            if game.genres.contains(&genre.id) {
                Some(genre.name.to_owned())
            } else {
                None
            }
        })
        .collect();

    let fmt = time::macros::format_description!("[month]/[year]");

    Review {
        title: game.name.clone(),
        link: game.url.clone(),
        cover_url: cover.url.clone(),
        date_released: game.first_release_date.map(|d| d.format(fmt).unwrap()),
        rating: gr.rating,
        description: gr.description.clone(),
        pros: gr.pros.clone(),
        cons: gr.cons.clone(),
        heart_count: gr.heart_count,
        genres,
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    // let cache = igdb::NoOpCache {};
    let sqlite_path = "game_reviews.sqlite3";
    let cache = igdb::SqliteCache::new(sqlite_path.to_string());
    let igdb = igdb::IGDB::new(cache).await?;
    let sections = get_sections(sqlite_path, &igdb).await?;
    let total_count: usize = sections.iter().map(|s| s.reviews.len()).sum();

    let mut wrt = std::io::BufWriter::new(std::io::stdout());
    let reviews = ReviewTemplate { sections };
    reviews.write_into(&mut wrt)?;
    log::info!("Generated reviews for {} games", total_count);

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
