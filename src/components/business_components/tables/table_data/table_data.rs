use crate::components::business_components::component::{
    repository_module::BRepository, BColumn, BColumnForeignKey, BCondition, BConstraint, BDataType,
    BRowColumnValue, BRowInsertData, BTableChangeEvents, BTableDataChangeEvents, BTableGeneral,
    BTableIn, BTableInfo, BTableInsertedData, BusinessComponent,
};
use crate::components::business_components::components::BusinessConsole;
use sqlx::Row;
use std::iter::zip;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task;

#[derive(Debug, Clone)]
pub struct TableData {
    repository: Arc<BRepository>,
    console: Arc<BusinessConsole>,
    pub tables_general_info: Arc<AsyncMutex<Vec<BTableGeneral>>>,
    pub table_inserted_data: Arc<AsyncMutex<Option<BTableInsertedData>>>,
    pub table_data_change_events: Arc<AsyncMutex<Vec<BTableDataChangeEvents>>>,
    primary_key_column_names: Arc<AsyncMutex<Vec<String>>>,
}
impl TableData {
    pub fn new(
        repository: Arc<BRepository>,
        console: Arc<BusinessConsole>,
        tables_general_info: Arc<AsyncMutex<Vec<BTableGeneral>>>,
    ) -> Self {
        Self {
            repository,
            console,
            tables_general_info,
            table_inserted_data: Arc::new(AsyncMutex::new(None)),
            table_data_change_events: Arc::new(AsyncMutex::new(vec![])),
            primary_key_column_names: Arc::new(AsyncMutex::new(vec![])),
        }
    }

    fn get_primary_key_conditions(
        &self,
        row_index: usize,
        table_data: &BTableInsertedData,
    ) -> Vec<BCondition> {
        let primary_key_columns = self.primary_key_column_names.blocking_lock();
        table_data
            .column_names
            .iter()
            .zip(&table_data.data_types)
            .zip(&table_data.rows[row_index])
            .filter(|((column_name, _), _)| primary_key_columns.contains(column_name))
            .map(|((column_name, data_type), value)| BCondition {
                column_name: column_name.clone(),
                data_type: data_type.clone(),
                value: value.clone(),
            })
            .collect()
    }

    fn find_existing_row_insert_event(
        &self,
        table_data_change_events: &[BTableDataChangeEvents],
        table_data: &BTableInsertedData,
        row_index: usize,
    ) -> Option<usize> {
        // Calculate the adjusted index for insertion events
        let delete_row_event_count = table_data_change_events
            .iter()
            .filter(|event| matches!(event, BTableDataChangeEvents::DeleteRow(_)))
            .count();
        let adjusted_index =
            row_index.checked_sub(table_data.rows.len() - delete_row_event_count)?;

        // Find the nth InsertRow event index
        table_data_change_events
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                if let BTableDataChangeEvents::InsertRow(_) = event {
                    Some(index)
                } else {
                    None
                }
            })
            .nth(adjusted_index)
    }
    pub fn add_insert_row_event(&self, values: Vec<String>) {
        let locked_table_data = self.table_inserted_data.blocking_lock();
        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();
        let table_data = locked_table_data.as_ref().unwrap();
        let row_index = table_data.rows.len();
        locked_table_data_change_events.push(BTableDataChangeEvents::InsertRow(BRowInsertData {
            column_names: table_data.column_names.clone(),
            values,
            data_types: table_data.data_types.clone(),
        }));
        self.console
            .write(format!("{:?}", locked_table_data_change_events));
    }

    pub fn add_modify_row_column_value_event(
        &self,
        row_index: usize,
        column_name: String,
        new_value: String,
    ) {
        // Step 1: Acquire the table data lock first, process what can be done without holding all locks
        let table_data = {
            let locked_table_data = self.table_inserted_data.blocking_lock();
            locked_table_data.as_ref().unwrap().clone() // Clone to minimize locking duration
        };

        // Step 3: Acquire necessary locks in a consistent order
        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();

        // Step 4: Check if there is an existing row insert event

        if let Some(existing_event_index) = self.find_existing_row_insert_event(
            &locked_table_data_change_events,
            &table_data,
            row_index,
        ) {
            if let Some(insert_row_event) =
                locked_table_data_change_events.get_mut(existing_event_index)
            {
                match insert_row_event {
                    BTableDataChangeEvents::InsertRow(row_insert_data) => {
                        row_insert_data.values =
                            zip(table_data.column_names, &row_insert_data.values)
                                .map(|(col_name, value)| {
                                    if col_name == column_name {
                                        new_value.clone() // Update the value for the matching column
                                    } else {
                                        value.clone() // Keep the existing value
                                    }
                                })
                                .collect();
                    }
                    _ => {} // Handle other enum variants if necessary
                }
            }
            self.console
                .write(format!("{:?}", locked_table_data_change_events));

            // Remove existing entries
            return;
        }

        // Step 2: Check if the row index is in the database
        if row_index >= table_data.rows.len() {
            return; // Invalid row index, no further processing needed
        }

        // Step 7: Proceed with new event creation
        let conditions = self.get_primary_key_conditions(row_index, &table_data);
        let column_datatype_index = table_data
            .column_names
            .iter()
            .position(|col_name| *col_name == column_name)
            .unwrap();
        let data_type = table_data.data_types[column_datatype_index].clone();
        let row_column_value = BRowColumnValue {
            conditions: conditions.clone(),
            column_name,
            new_value,
            data_type,
        };

        // Step 8: Check for existing event and replace if necessary
        if let Some(existing_event_index) = locked_table_data_change_events
        .iter()
        .position(|existing_event| {
            matches!(
                existing_event,
                BTableDataChangeEvents::ModifyRowColumnValue(existing_row_column_value)
                if zip(&row_column_value.conditions, &existing_row_column_value.conditions)
                    .all(|(condition, existing_condition)| condition.value == existing_condition.value)
            )
        })
    {
        locked_table_data_change_events.remove(existing_event_index);
    }

        // Add the new event
        locked_table_data_change_events.push(BTableDataChangeEvents::ModifyRowColumnValue(
            row_column_value,
        ));
        self.console
            .write(format!("{:?}", locked_table_data_change_events));
    }

    pub fn add_delete_row_event(&self, row_index: usize) {
        // Acquire locks for necessary data
        let locked_table_data = self.table_inserted_data.blocking_lock();

        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();

        // Safely unwrap the locked data
        let table_data = locked_table_data.as_ref().unwrap();

        if let Some(existing_event_index) = self.find_existing_row_insert_event(
            &locked_table_data_change_events,
            &table_data,
            row_index,
        ) {
            println!("removing table insert event");
            locked_table_data_change_events.remove(existing_event_index);
            self.console
                .write(format!("{:?}", locked_table_data_change_events));

            return;
        }
        // Ensure the row index is valid
        else if row_index >= table_data.rows.len() {
            return; // Exit if the row index is out of bounds
        }

        // Extract conditions based on primary key column names
        let conditions = self.get_primary_key_conditions(row_index, &table_data);

        // Add the delete row event
        locked_table_data_change_events.push(BTableDataChangeEvents::DeleteRow(conditions));

        // Log the current state of table data change events to the console
        self.console
            .write(format!("{:?}", *locked_table_data_change_events));
    }

    pub async fn update_table_data(&self) {
        // Extract and drop the lock on `table_inserted_data`
        let (table_name, table_data_change_events) = {
            let table_inserted_data_guard = self.table_inserted_data.lock().await;
            if let Some(ref table_inserted_data) = *table_inserted_data_guard {
                let table_name = table_inserted_data.table_name.clone();
                let table_data_change_events_guard = self.table_data_change_events.lock().await;
                let table_data_change_events = table_data_change_events_guard.clone();
                (table_name, table_data_change_events)
            } else {
                return; // If there's no table_inserted_data, exit the function
            }
        };
        {
            let locked_primary_key_column_names = self.primary_key_column_names.lock().await;
            // Use the extracted values without holding the locks
            self.repository
                .update_table_data(
                    &table_name,
                    &table_data_change_events,
                    &locked_primary_key_column_names,
                )
                .await;
        }
        self.set_table_data(table_name).await;
    }
    pub async fn set_table_data(&self, table_name: String) {
        // Lock the general info table
        let tables_general_info = self.tables_general_info.lock().await;
        if let Some(table_general_info) = tables_general_info
            .iter()
            .find(|info| info.table_name == table_name)
        {
            let primary_key_column_names = self
                .repository
                .get_primary_key_column_names(&table_name)
                .await
                .unwrap();
            // Fetch rows for the table
            let table_data_rows = self
                .repository
                .get_table_data_rows(
                    &table_name,
                    &table_general_info.column_names,
                    &primary_key_column_names,
                )
                .await
                .unwrap();
            // Construct the inserted data
            let table_inserted_data = BTableInsertedData {
                table_name: table_name.clone(),
                column_names: table_general_info.column_names.clone(),
                data_types: table_general_info.data_types.clone(),
                rows: table_data_rows
                    .iter()
                    .map(|row| {
                        table_general_info
                            .column_names
                            .iter()
                            .map(|column_name| row.get::<String, _>(column_name.as_str()))
                            .collect::<Vec<String>>()
                    })
                    .collect::<Vec<Vec<String>>>(),
            };
            // Update the shared table inserted data
            *self.table_inserted_data.lock().await = Some(table_inserted_data);
            *self.table_data_change_events.lock().await = vec![];
            *self.primary_key_column_names.lock().await = primary_key_column_names;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::business_components::component::{
        repository_module::BRepositoryConsole, BTableGeneral, BTableIn,
    };
    use crate::components::business_components::tables::{
        test_utils::{
            create_btable_general, create_repository_table_and_console, default_table_in,
            sort_columns, sort_tables_general_info,
        },
        utils::set_tables_general_info,
    };
    use sqlx::PgPool;

    async fn create_table_data(
        pool: PgPool,
        table_in: &BTableIn,
        tables_general_info: Arc<AsyncMutex<Vec<BTableGeneral>>>,
    ) -> TableData {
        let (repository_result, console_result) =
            create_repository_table_and_console(pool, table_in).await;
        let table_general_info = Arc::new(AsyncMutex::new(Vec::<BTableGeneral>::new()));
        set_tables_general_info(repository_result.clone(), tables_general_info.clone()).await;
        let table_data = TableData::new(repository_result, console_result, tables_general_info);
        table_data
    }

    #[sqlx::test]
    async fn test_insert_data_into_table(pool: PgPool) {
        let tables_general_info: Arc<AsyncMutex<Vec<BTableGeneral>>> =
            Arc::new(AsyncMutex::new(Vec::new()));
        let table_in = default_table_in();
        let table_data = create_table_data(pool, &table_in, tables_general_info).await;
    }
}
