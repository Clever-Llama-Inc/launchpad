#[cfg(test)]
mod tests {
    use launchpad_derive::Entity;
    use uuid::Uuid;

    #[allow(unused)]
    #[derive(Entity, Default)]
    #[entity(name = my_entity)]
    struct MyEntity {
        #[key(name = "id")]
        #[column(name = "id")]
        entity_id: Uuid,

        #[key(name = "name_version")]
        name: String,

        #[key(name = "name_version")]
        version: i32,

        description: String,
    }

    #[test]
    fn usage() {
        let _entity = MyEntity::default();
        // impl MyEntityRepo for () {}
    }
}
