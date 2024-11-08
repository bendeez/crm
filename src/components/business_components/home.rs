use crate::components::business_components::component::{
    repository_module::BusinessRepository, BusinessComponent, BusinessTableOut,
};

#[derive(Debug, Clone)]
pub struct Home {
    repository: BusinessRepository,
    pub title: Option<String>,
    pub tables: Option<Vec<BusinessTableOut>>,
}

impl BusinessComponent for Home {
    async fn initialize_component(&mut self) {
        self.tables = Some(self.repository.get_tables().await.unwrap());
        self.title = Some(String::from("Home Component"));
    }
}

impl Home {
    pub fn new(repository: BusinessRepository) -> Self {
        Self {
            repository,
            title: None,
            tables: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::PgPool;

    async fn home_business_component(pool: PgPool) -> Home {
        let repository = BusinessRepository::new(Some(pool)).await;
        Home {
            repository,
            title: None,
            tables: None,
        }
    }

    #[sqlx::test]
    async fn test_initialize_home_component(pool: PgPool) {
        sqlx::query!("CREATE TABLE users (name TEXT)")
            .execute(&pool)
            .await
            .unwrap();
        let mut home = home_business_component(pool).await;
        home.initialize_component().await;
        let expected_tables = vec![BusinessTableOut {
            table_name: String::from("users"),
        }];

        assert_eq!(home.tables, Some(expected_tables));
        assert_eq!(home.title, Some(String::from("Home Component")));
    }
}
