use crate::components::business_components::component::{
    repository_module::BRepository, BColumn, BConstraint, BDataType, BTableChangeEvents,
    BTableGeneral, BTableIn, BTableInfo, BTableInsertedData, BusinessComponent,
};
use crate::components::business_components::components::BusinessConsole;
use crate::components::business_components::tables::table_info::TableInfo;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Clone)]
pub struct Tables {
    repository: Arc<BRepository>,
    pub table_info: Arc<TableInfo>,
    pub tables_general_info: Arc<AsyncMutex<Vec<BTableGeneral>>>,
    console: Arc<BusinessConsole>,
}

impl BusinessComponent for Tables {
    async fn initialize_component(&self) {
        self.set_general_tables_info().await;
    }
}

impl Tables {
    pub fn new(repository: Arc<BRepository>, console: Arc<BusinessConsole>) -> Self {
        let tables_general_info = Arc::new(AsyncMutex::new(vec![]));
        Self {
            table_info: Arc::new(BTableInfo::new(
                repository.clone(),
                console.clone(),
                tables_general_info.clone(),
            )),
            repository,
            tables_general_info,
            console,
        }
    }

    async fn set_general_tables_info(&self) {
        let mut locked_tables = self.tables_general_info.lock().await;
        let tables_general_info = self.repository.get_general_tables_info().await.unwrap();
        let tables = tables_general_info
            .into_iter()
            .map(|table| BTableGeneral::to_table(table))
            .collect();
        *locked_tables = tables;
    }

    pub async fn add_table(&self, table_in: BTableIn) {
        self.repository.create_table(&table_in).await;
        self.set_general_tables_info().await;
    }

    pub async fn delete_table(&self, table_name: String) {
        self.repository.delete_table(&table_name).await;
        self.set_general_tables_info().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::business_components::component::repository_module::BRepositoryConsole;
    use sqlx::PgPool;

    async fn tables_component(pool: PgPool, table_in: &BTableIn) -> Tables {
        let database_console = Arc::new(BRepositoryConsole::new());

        let repository = Arc::new(BRepository::new(Some(pool), database_console.clone()).await);
        repository.create_table(table_in).await;

        let console = Arc::new(BusinessConsole::new(database_console));
        Tables::new(repository, console)
    }

    async fn initialized_tables_component(pool: PgPool, table_in: &BTableIn) -> Tables {
        let tables = tables_component(pool, table_in).await;
        tables.initialize_component().await;
        tables
    }

    fn default_table_in() -> BTableIn {
        BTableIn {
            table_name: String::from("users"),
            columns: vec![BColumn {
                name: String::from("name"),
                datatype: BDataType::TEXT,
                constraints: vec![BConstraint::PrimaryKey],
            }],
        }
    }

    fn create_btable_general(table_in: &BTableIn) -> BTableGeneral {
        BTableGeneral {
            table_name: table_in.table_name.clone(),
            column_names: table_in
                .columns
                .iter()
                .map(|col| col.name.clone())
                .collect(),
            data_types: table_in
                .columns
                .iter()
                .map(|col| col.datatype.clone())
                .collect(),
        }
    }

    fn sort_by_table_name(tables: &mut Vec<BTableGeneral>) {
        tables.sort_by(|a, b| a.table_name.cmp(&b.table_name));
    }

    #[sqlx::test]
    async fn test_initialize_tables_component(pool: PgPool) {
        let table_in = default_table_in();
        let tables = initialized_tables_component(pool, &table_in).await;

        let mut expected_tables_general_info = vec![create_btable_general(&table_in)];
        let mut tables_general_info = tables.tables_general_info.lock().await.clone();

        // Sort both vectors
        sort_by_table_name(&mut expected_tables_general_info);
        sort_by_table_name(&mut tables_general_info);

        assert_eq!(tables_general_info, expected_tables_general_info);
    }

    #[sqlx::test]
    async fn test_add_table(pool: PgPool) {
        let initial_table_in = default_table_in();
        let tables = initialized_tables_component(pool, &initial_table_in).await;

        let new_table_in = BTableIn {
            table_name: String::from("products"),
            columns: vec![BColumn {
                name: String::from("product_name"),
                datatype: BDataType::TEXT,
                constraints: vec![],
            }],
        };

        // Add a new table
        tables.add_table(new_table_in.clone()).await;

        // Prepare expected results
        let mut expected_tables_general_info = vec![
            create_btable_general(&initial_table_in),
            create_btable_general(&new_table_in),
        ];

        let mut tables_general_info = tables.tables_general_info.lock().await.clone();

        // Sort both vectors
        sort_by_table_name(&mut expected_tables_general_info);
        sort_by_table_name(&mut tables_general_info);

        assert_eq!(tables_general_info, expected_tables_general_info);
    }

    #[sqlx::test]
    async fn test_delete_table(pool: PgPool) {
        let table_in = default_table_in();
        let tables = initialized_tables_component(pool, &table_in).await;

        // Delete the initial table
        tables.delete_table(table_in.table_name.clone()).await;

        // Verify no tables exist in `tables_general_info`
        let tables_general_info = tables.tables_general_info.lock().await;
        assert!(tables_general_info.is_empty());
    }
}
