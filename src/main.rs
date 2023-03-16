use sqlx::Connection;
use askama::Template;

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

struct Section {
    category: Category,
    games: Vec<Game>
}

#[derive(Template)]
#[template(path="reviews.html")]
struct ReviewTemplate {
    sections: Vec<Section>
}

mod filters {
    use std::fmt::Write;

    /// for the heart count
    pub fn repeat<T: std::fmt::Display>(s: T, count: &&i64) -> askama::Result<String> {
        let count = (**count).try_into().unwrap();
        Ok(s.to_string().repeat(count))
    }

    pub fn split_tags2<'a>(raw: &'a &String) -> askama::Result<Vec<&'a str>> {
        Ok(raw.split("/").into_iter().collect())
    }

    pub fn split_tags(tags: &&String) -> askama::Result<String> {
        if tags.is_empty() {
            return Ok("".to_string())
        }

        let mut wrt = String::from(r#"<ul class="tags">"#);
        for tag in tags.split("/") {
            write!(&mut wrt, "<li>{}</li>", tag)?;
        }
        write!(&mut wrt, "</ul>")?;
        Ok(wrt)
    }
}


#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut conn = sqlx::SqliteConnection::connect("game_reviews.sqlite3").await?;
    // let mut wrt = tokio::io::BufWriter::new(tokio::io::stdout());
    let mut wrt = std::io::BufWriter::new(std::io::stdout());

    let categories = sqlx::query_as::<_, Category>("SELECT * from category ORDER BY sort_order")
        .fetch_all(&mut conn)
        .await?;

    let mut sections = Vec::with_capacity(categories.len());

    for cat in categories {

        let games = sqlx::query_as::<_, Game>(
            "SELECT game.*, GROUP_CONCAT(tag.value, '/') as tags
            FROM game
            LEFT OUTER JOIN game_tag ON game_tag.game_id = game.id
            LEFT OUTER JOIN tag ON game_tag.tag_id = tag.id
            WHERE game.category_id = ?
            GROUP BY game.id
            ORDER BY game.rating DESC, game.title",
        )
        .bind(cat.id)
        .fetch_all(&mut conn)
        .await?;

        sections.push(Section{
            category: cat,
            games
        });
    }

    let reviews = ReviewTemplate{sections};
    reviews.write_into(&mut wrt)?;

    Ok(())
}
