pub mod types {

    pub mod public {
        use postgres_types::{FromSql, ToSql};

        #[derive(Debug, ToSql, FromSql)]
        #[postgres(name = "spongebob_character")]
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub enum SpongebobCharacter {
            Bob,
            Patrick,
            Squidward,
        }

        #[derive(Debug, ToSql, FromSql)]
        #[postgres(name = "custom_composite")]
        #[derive(Clone)]
        pub struct CustomComposite {
            pub such_cool: i32,
            pub wow: String,
            pub nice: super::public::SpongebobCharacter,
        }
    }
}
pub mod queries {
    pub mod module_1 {

        use cornucopia_client::GenericClient;
        use tokio_postgres::Error;

        pub async fn insert_book_one<T: GenericClient>(client: &T) -> Result<(), Error> {
            let stmt = client
                .prepare(
                    "INSERT INTO Book (title)
VALUES ('bob');
",
                )
                .await?;
            let _ = client.execute(&stmt, &[]).await?;

            Ok(())
        }

        pub async fn insert_book_zero_or_one<T: GenericClient>(client: &T) -> Result<(), Error> {
            let stmt = client
                .prepare(
                    "INSERT INTO Book (title)
VALUES ('alice');
",
                )
                .await?;
            let _ = client.execute(&stmt, &[]).await?;

            Ok(())
        }

        pub async fn insert_book_zero_or_more<T: GenericClient>(client: &T) -> Result<(), Error> {
            let stmt = client
                .prepare(
                    "INSERT INTO Book (title)
VALUES ('carl');
",
                )
                .await?;
            let _ = client.execute(&stmt, &[]).await?;

            Ok(())
        }
    }

    pub mod module_2 {

        use cornucopia_client::GenericClient;
        use tokio_postgres::Error;

        pub async fn authors<T: GenericClient>(
            client: &T,
        ) -> Result<Vec<(i32, String, String)>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
*
FROM
Author;
",
                )
                .await?;
            let res = client.query(&stmt, &[]).await?;

            let return_value = res
                .iter()
                .map(|res| {
                    let return_value_0: i32 = res.get(0);
                    let return_value_1: String = res.get(1);
                    let return_value_2: String = res.get(2);
                    (return_value_0, return_value_1, return_value_2)
                })
                .collect::<Vec<(i32, String, String)>>();
            Ok(return_value)
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct Books {
            pub title: String,
        }
        pub async fn books<T: GenericClient>(
            client: &T,
        ) -> Result<Vec<super::super::queries::module_2::Books>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
Title
FROM
Book;
",
                )
                .await?;
            let res = client.query(&stmt, &[]).await?;

            let return_value = res
                .iter()
                .map(|res| {
                    let return_value_0: String = res.get(0);
                    super::super::queries::module_2::Books {
                        title: return_value_0,
                    }
                })
                .collect::<Vec<super::super::queries::module_2::Books>>();
            Ok(return_value)
        }

        #[derive(Debug, Clone, PartialEq)]
        pub struct BooksOptRetParam {
            pub title: Option<String>,
        }
        pub async fn books_opt_ret_param<T: GenericClient>(
            client: &T,
        ) -> Result<Vec<super::super::queries::module_2::BooksOptRetParam>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
Title
FROM
Book;
",
                )
                .await?;
            let res = client.query(&stmt, &[]).await?;

            let return_value = res
                .iter()
                .map(|res| {
                    let return_value_0: Option<String> = res.get(0);
                    super::super::queries::module_2::BooksOptRetParam {
                        title: return_value_0,
                    }
                })
                .collect::<Vec<super::super::queries::module_2::BooksOptRetParam>>();
            Ok(return_value)
        }

        pub async fn books_from_author_id<T: GenericClient>(
            client: &T,
            id: &i32,
        ) -> Result<Vec<String>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
Book.Title
FROM
BookAuthor
INNER JOIN Author ON Author.Id = BookAuthor.AuthorId
INNER JOIN Book ON Book.Id = BookAuthor.BookId
WHERE
Author.Id = $1;
",
                )
                .await?;
            let res = client.query(&stmt, &[&id]).await?;

            let return_value = res
                .iter()
                .map(|row| {
                    let value: String = row.get(0);
                    value
                })
                .collect::<Vec<String>>();
            Ok(return_value)
        }

        pub async fn author_name_by_id_opt<T: GenericClient>(
            client: &T,
            id: &i32,
        ) -> Result<Option<String>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
Author.Name
FROM
Author
WHERE
Author.Id = $1;
",
                )
                .await?;
            let res = client.query_opt(&stmt, &[&id]).await?;

            let return_value = res.map(|row| {
                let value: String = row.get(0);
                value
            });
            Ok(return_value)
        }

        pub async fn author_name_by_id<T: GenericClient>(
            client: &T,
            id: &i32,
        ) -> Result<String, Error> {
            let stmt = client
                .prepare(
                    "SELECT
Author.Name
FROM
Author
WHERE
Author.Id = $1;
",
                )
                .await?;
            let res = client.query_one(&stmt, &[&id]).await?;

            let return_value: String = res.get(0);
            Ok(return_value)
        }

        pub async fn author_name_starting_with<T: GenericClient>(
            client: &T,
            s: &str,
        ) -> Result<Vec<(i32, String, i32, String)>, Error> {
            let stmt = client
                .prepare(
                    "SELECT
BookAuthor.AuthorId,
Author.Name,
BookAuthor.BookId,
Book.Title
FROM
BookAuthor
INNER JOIN Author ON Author.id = BookAuthor.AuthorId
INNER JOIN Book ON Book.Id = BookAuthor.BookId
WHERE
Author.Name LIKE CONCAT($1::text, '%');
",
                )
                .await?;
            let res = client.query(&stmt, &[&s]).await?;

            let return_value = res
                .iter()
                .map(|res| {
                    let return_value_0: i32 = res.get(0);
                    let return_value_1: String = res.get(1);
                    let return_value_2: i32 = res.get(2);
                    let return_value_3: String = res.get(3);
                    (
                        return_value_0,
                        return_value_1,
                        return_value_2,
                        return_value_3,
                    )
                })
                .collect::<Vec<(i32, String, i32, String)>>();
            Ok(return_value)
        }

        pub async fn return_custom_type<T: GenericClient>(
            client: &T,
        ) -> Result<super::super::types::public::CustomComposite, Error> {
            let stmt = client
                .prepare(
                    "SELECT
col1
FROM
CustomTable;
",
                )
                .await?;
            let res = client.query_one(&stmt, &[]).await?;

            let return_value: super::super::types::public::CustomComposite = res.get(0);
            Ok(return_value)
        }

        pub async fn select_where_custom_type<T: GenericClient>(
            client: &T,
            spongebob_character: &super::super::types::public::SpongebobCharacter,
        ) -> Result<super::super::types::public::SpongebobCharacter, Error> {
            let stmt = client
                .prepare(
                    "SELECT
col2
FROM
CustomTable
WHERE (col1).nice = $1;
",
                )
                .await?;
            let res = client.query_one(&stmt, &[&spongebob_character]).await?;

            let return_value: super::super::types::public::SpongebobCharacter = res.get(0);
            Ok(return_value)
        }
    }
}
