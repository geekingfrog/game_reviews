use futures::StreamExt;
use sqlx::{Connection, Executor};
use std::fmt::Write;
use tokio::io::AsyncWriteExt;

#[derive(sqlx::FromRow, Debug)]
struct Game {
    id: i64,
    title: String,
    link: Option<String>,
    year_played: Option<String>,
    year_released: i64,
    rating: Option<i64>,
    description: String,
    pros: Option<String>,
    cons: Option<String>,
    category_id: i64,
    heart_count: Option<i64>,
}

impl Game {
    /// generate some markdown intented to be printed on github
    fn to_markdown(&self) -> anyhow::Result<String> {
        let mut result = String::new();
        write!(&mut result, "*")?;

        match &self.link {
            Some(link) => write!(&mut result, "[{}]({})", self.title, link)?,
            None => write!(&mut result, "**{}**", self.title)?,
        };

        match &self.year_played {
            Some(y) => write!(&mut result, " - joué en {y} ")?,
            None => (),
        };

        let heart_count: usize = self.heart_count.unwrap_or_default().try_into().expect(
            &format!(
                "invalid heart_count for game id {}: {:?}",
                self.id, self.heart_count
            ),
        );
        if heart_count > 0 {
            write!(&mut result, "{}", ":heart:".repeat(heart_count))?;
        }

        if let Some(r) = self.rating {
            write!(&mut result, "**{r}/20**")?;
        };

        write!(&mut result, "\n*Sorti en {}*\n", self.year_released)?;
        // TODO the genres/tags here

        write!(&mut result, ":information_source: {}\n", self.description)?;

        if let Some(pros) = &self.pros {
            write!(&mut result, ":heavy_check_mark: {pros}\n")?;
        };

        if let Some(cons) = &self.cons {
            write!(&mut result, ":x: {cons}\n")?;
        };

        Ok(result)
    }
}

#[derive(sqlx::FromRow, Debug)]
struct Category {
    id: i64,
    title: String,
    sort_order: i64,
    description: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    println!("Hello, world!");

    let mut conn = sqlx::SqliteConnection::connect("game_reviews.sqlite3").await?;
    let mut wrt = tokio::io::BufWriter::new(tokio::io::stdout());

    let categories = sqlx::query_as::<_, Category>("SELECT * from category ORDER BY sort_order")
        .fetch_all(&mut conn)
        .await?;

    for cat in categories {
        wrt.write(format!("# {}\n{}\n\n", cat.title, cat.description).as_bytes())
            .await?;

        let games = sqlx::query_as::<_, Game>(
            "SELECT * FROM game WHERE category_id = ? ORDER BY rating DESC, title LIMIT 8",
        )
        .bind(cat.id)
        .fetch_all(&mut conn)
        .await?;

        for game in games {
            wrt.write(game.to_markdown()?.as_bytes()).await?;
            wrt.write("\n\n".as_bytes()).await?;
        }
    }

    // let mut games = sqlx::query_as::<_, Game>("SELECT * from game LIMIT ?")
    //     .bind(9999)
    //     .fetch(&mut conn);

    // while let Some(game) = games.next().await.transpose()? {
    //     println!("{} - {}", game.id, game.title);
    // }

    wrt.flush().await?;
    Ok(())
}
