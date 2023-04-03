use std::collections::{BTreeMap, BTreeSet};

use askama::Template;
use sqlx::Connection;

use game_reviews::igdb;

#[derive(sqlx::FromRow, Debug)]
struct Game {
    #[allow(dead_code)]
    id: i64,
    title: String,
    link: Option<String>,
    year_played: Option<String>,
    year_released: i64,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    #[allow(dead_code)]
    category_id: i64,
    heart_count: Option<i64>,
    tags: Option<String>,
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
    igdb_id: i64,
    title: String,
    year_played: Option<String>,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    #[allow(dead_code)]
    category_id: i64,
    heart_count: Option<i64>,
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

    pub fn split_tags<'a>(raw: &'a &String) -> askama::Result<Vec<&'a str>> {
        Ok(raw.split("/").into_iter().collect())
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    // populate().await?;

    // let cache = igdb::NoOpCache {};
    let cache = igdb::SqliteCache::new("game_reviews.sqlite3".to_string());
    let igdb = igdb::IGDB::new(cache).await?;

    let games = igdb.get_games(&[7046,71]).await?;
    for g in &games {
        println!("{g:#?}");
    }

    let genre_ids = games.iter().fold(BTreeSet::new(), |mut genres, g| {
        for genre in &g.genres {
            genres.insert(*genre);
        }
        genres
    });

    let genre_ids = genre_ids.into_iter().collect::<Vec<_>>();
    let genres = igdb.get_genres(&genre_ids[..]).await?;
    for g in &genres {
        println!("{g:#?}");
    }

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

/// temp, to copy data from manually populated DB to a different table
/// and make use of igdb for some metadata
#[allow(dead_code)]
async fn populate() -> anyhow::Result<()> {
    let mut conn = sqlx::SqliteConnection::connect("game_reviews.sqlite3").await?;

    sqlx::query("delete from igdb_cache")
        .execute(&mut conn)
        .await?;

    let id_mappings = BTreeMap::from([
        ("Invisible, Inc.", 6044),
        ("The legend of Zelda, majora's mask", 1030),
        ("The legend of Zelda, ocarina of time", 1029),
        ("Factorio", 7046),
        ("Hollow knight", 14593),
        ("Portal", 71),
        ("Portal 2", 72),
        ("Celeste", 26226),
        ("Dead cell", 26855),
        ("Fire emblem three houses", 26845),
        ("Hades", 113112),
        ("Life is Strange", 7599),
        ("Metroid dread", 15698),
        ("Metroid prime", 1105),
        ("Metroid prime echoes", 1108),
        ("Slay the spire", 40477),
        ("Supreme commander forged alliance forever (faf)", 142868),
        ("The legend of Zelda: breath of the wild", 7346),
        ("Xenoblade chronicles", 2364),
        ("Xenoblade chronicles 2", 26766),
        ("Xenoblade chronicles 2 DLC: Torna", 103340),
        ("Age of wonders 3", 5652),
        ("Cross code", 35282),
        ("Dishonored", 533),
        ("DokiDoki literature club", 152122),
        ("Donkey Kong country: tropical freeze", 85576),
        ("FTL: faster than light", 20098),
        ("Finding paradise", 36044),
        ("Fire emblem awakening", 1443),
        ("Into the breach", 27117),
        ("Mark of the ninja remastered", 94969),
        ("Ori and the blind forest", 19456),
        ("Overcooked", 18433),
        ("Rivals of aether", 21646),
        ("Super hexagon", 3251),
        ("Super mario odyssey", 26758),
        ("Super smash brother ultimate", 90101),
        ("The legend of Zelda: a link between worlds", 2909),
        ("The legend of Zelda: the windwaker", 1033),
        ("Timberborn", 126381),
        ("XCOM - enemy unknown", 1318),
        ("Furi", 17026),
        ("Iconoclasts", 34705),
        ("League of Legends", 115),
        ("Magicka 2", 9807),
        ("Mini metro", 7767),
        ("Opus magnum", 74545),
        ("Stellaris", 11582),
        ("The talos principle", 7386),
        ("To the moon", 5025),
        ("Transistor", 3022),
        ("Wizard of legend", 19935),
        ("Xenoblade chronicles X", 2366),
        ("Bastion", 1983),
        ("Creeper world 3: arc eternal", 9809),
        ("Geometry dash", 11642),
        ("Geometry wars 3", 7705),
        ("Mindustry", 83368),
        ("Northgard", 18918),
        ("SpaceChem", 8390),
        ("Star traders: frontiers", 74163),
        ("Super auto pets", 146641),
        ("Pillars of eternity", 1593),
        ("Pyre", 18822),
        ("Convoy", 9921),
        ("Renowned explorers: international society", 12510),
        ("Metroid Samus returns", 37140),
        ("Rimworld", 9789),
        ("The swapper", 5892),
        ("Baldur's gate", 5),
        ("DOTA", 2283),
        ("Diablo 1 & 2 + lord of destruction", 246),
        ("Heroes of might and magic 3", 20052),
        ("Rollercoaster tycoon", 254),
        ("Starcraft", 230),
        ("Total Annihilation", 918),
        ("Warcraft 2", 130),
        ("Warcraft 3", 133),
    ]);

    let games = sqlx::query_as::<_, Game>(
        "SELECT game.*, GROUP_CONCAT(tag.value, '/') as tags
        FROM game
        LEFT OUTER JOIN game_tag ON game_tag.game_id = game.id
        LEFT OUTER JOIN tag ON game_tag.tag_id = tag.id
        GROUP BY game.id
        ORDER BY game.rating DESC, game.title",
    )
    .fetch_all(&mut conn)
    .await?;

    for g in games {
        let igdb_id = id_mappings[&g.title[..]];
        sqlx::query(
            r#"INSERT INTO game_review (igdb_id, title, year_played,
        rating, description, pros, cons, heart_count, category_id)
        VALUES (?,?,?,?,?,?,?,?,?)
        "#,
        )
        .bind(igdb_id)
        .bind(g.title)
        .bind(g.year_played)
        .bind(g.rating)
        .bind(g.description)
        .bind(g.pros)
        .bind(g.cons)
        .bind(g.heart_count)
        .bind(g.category_id)
        .execute(&mut conn)
        .await?;
    }
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
