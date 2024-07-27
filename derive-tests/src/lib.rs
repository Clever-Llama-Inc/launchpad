#[cfg(test)]
mod tests {
    use std::iter;

    use itertools::Itertools;
    use launchpad_derive::Entity;
    use sqlx::{prelude::FromRow, PgPool};
    use uuid::Uuid;

    #[allow(unused)]
    #[derive(Entity, Default, FromRow, Debug)]
    #[entity(name = my_entity, table_name = "my_entity")]
    struct MyEntity {
        #[key(name = "id", unique)]
        #[column(name = "id")]
        #[sqlx(rename = "id")]
        entity_id: Uuid,

        #[key(name = "name_version", unique)]
        name: String,

        #[key(name = "name_version", unique)]
        version: i32,

        #[key(name = "color")]
        color: String,

        description: String,
    }

    #[test]
    fn usage() {
        let _entity = MyEntity::default();
        // impl MyEntityRepo for () {}
    }

    #[tokio::test]
    async fn integration() -> Result<(), sqlx::Error> {
        let pg_pool = if let Ok(pg_url) = std::env::var("PG_URL") {
            PgPool::connect(&pg_url).await?
        } else {
            eprintln!("No $PG_URL found. skipping.");
            return Ok(());
        };

        let result: Result<(), sqlx::Error> = async {
            create_table(&pg_pool).await?;
            let (id1, id2, id3) = iter::from_fn(|| Some(Uuid::new_v4())).take(3).collect_tuple().unwrap();
            let data = vec![
                MyEntity {
                    entity_id: id1,
                    name: "foo".into(),
                    version: 1,
                    color: "red".into(),
                    description: "foo red".into(),
                },
                MyEntity {
                    entity_id: id2,
                    name: "foo".into(),
                    version: 2,
                    color: "blue".into(),
                    description: "foo blue".into(),
                },
                MyEntity {
                    entity_id: id3,
                    name: "bar".into(),
                    version: 1,
                    color: "red".into(),
                    description: "bar red".into(),
                },
            ];
            
            for entity in &data {
                insert_my_entity(&pg_pool, entity).await?;
            }

            let e1 = pg_pool.find_my_entity_by_id(&id1).await?;
            assert!(e1.is_some());
            assert_eq!(e1.unwrap().description, "foo red" );

            let e2 = pg_pool.list_my_entity_by_color("red").await?;
            assert_eq!(e2.len(), 2);

            Ok(())
        }
        .await;

        drop_table(&pg_pool).await?;

        Ok(result?)
    }

    async fn create_table(pg_pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query(
            "create table if not exists my_entity (
            id uuid primary key,
            name text not null,
            version integer not null,
            color text,
            description text,
            unique(name, version)
        );",
        )
        .execute(pg_pool)
        .await?;
        Ok(())
    }

    async fn drop_table(pg_pool: &PgPool) -> Result<(), sqlx::Error> {
        sqlx::query("drop table my_entity").execute(pg_pool).await?;
        Ok(())
    }

    async fn insert_my_entity(pg_pool: &PgPool, entity: &MyEntity) -> Result<(), sqlx::Error> {
        let values = (1..=5).map(|i| format!("${i}")).collect_vec().join(", ");
        sqlx::query(&format!("insert into my_entity (id, name, version, color, description) values ({values})"))
        .bind(&entity.entity_id)
        .bind(&entity.name)
        .bind(&entity.version)
        .bind(&entity.color)
        .bind(&entity.description)
        .execute(pg_pool)
        .await?;
        Ok(())
    }
}
