#[cfg(test)]
mod tests {
    use launchpad_derive::Entity;
    use sqlx::prelude::FromRow;
    use uuid::Uuid;

    #[allow(unused)]
    #[derive(Entity, Default, FromRow)]
    #[entity(name = my_entity)]
    struct MyEntity {
        #[key(name = "id", unique)]
        #[column(name = "id")]
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
    
}
