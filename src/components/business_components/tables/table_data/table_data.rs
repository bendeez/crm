use crate::components::business_components::component::{
    repository_module::BRepository, BColumn, BColumnForeignKey, BCondition, BConstraint, BDataType,
    BRowColumnValue, BRowInsertData, BTableChangeEvents, BTableDataChangeEvents, BTableGeneral,
    BTableIn, BTableInfo, BTableInsertedData, BusinessComponent,
};
use crate::components::business_components::components::BusinessConsole;
use sqlx::Row;
use std::collections::HashMap;
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
    table_data_change_events: Arc<AsyncMutex<Vec<BTableDataChangeEvents>>>,
    primary_key_column_names: Arc<AsyncMutex<Vec<String>>>,
    current_to_initial_row_indexes: Arc<AsyncMutex<HashMap<usize, usize>>>,
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
            current_to_initial_row_indexes: Arc::new(AsyncMutex::new(HashMap::new())),
        }
    }

    pub fn reset_table_data(&self) {
        let mut locked_table_inserted_data = self.table_inserted_data.blocking_lock();
        *locked_table_inserted_data = None;
        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();
        *locked_table_data_change_events = vec![];
        let mut locked_primary_key_column_names = self.primary_key_column_names.blocking_lock();
        *locked_primary_key_column_names = vec![];
        let mut locked_current_to_initial_row_indexes =
            self.current_to_initial_row_indexes.blocking_lock();
        *locked_current_to_initial_row_indexes = HashMap::new();
    }

    fn get_primary_key_conditions(
        &self,
        row_index: usize,
        table_inserted_data: &BTableInsertedData,
    ) -> Vec<BCondition> {
        let primary_key_column_names = self.primary_key_column_names.blocking_lock();
        let adjusted_row_index = self
            .current_to_initial_row_indexes
            .blocking_lock()
            .get(&row_index)
            .unwrap()
            .clone();
        table_inserted_data
            .column_names
            .iter()
            .zip(&table_inserted_data.data_types)
            .zip(&table_inserted_data.rows[adjusted_row_index])
            .filter(|((column_name, _), _)| primary_key_column_names.contains(column_name))
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
        table_inserted_data: &BTableInsertedData,
        row_index: usize,
    ) -> Option<usize> {
        // Calculate the adjusted index for insertion events
        let delete_row_event_count = table_data_change_events
            .iter()
            .filter(|event| matches!(event, BTableDataChangeEvents::DeleteRow(_)))
            .count();
        // checks if row was inserted through a insert row event
        // by row_index >= len(rows) - deleted row events
        let index_of_events_filtered_by_insert_row =
            row_index.checked_sub(table_inserted_data.rows.len() - delete_row_event_count)?;

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
            .nth(index_of_events_filtered_by_insert_row)
    }

    fn find_existing_modify_row_event(
        &self,
        table_data_change_events: &[BTableDataChangeEvents],
        table_inserted_data: &BTableInsertedData,
        row_index: usize,
    ) -> Option<usize> {
        let conditions = self.get_primary_key_conditions(row_index, table_inserted_data);
        table_data_change_events.iter().position(|event| matches!(event, BTableDataChangeEvents::ModifyRowColumnValue(row_column_value) if row_column_value.conditions == conditions))
    }

    fn update_modify_row_event(
        &self,
        table_data_change_events: &mut Vec<BTableDataChangeEvents>,
        table_inserted_data: &BTableInsertedData,
        row_index: usize,
        event_index: usize,
        column_name: String,
        new_value: String,
        data_type: BDataType,
    ) {
        if let Some(event) = table_data_change_events.get_mut(event_index) {
            match event {
                BTableDataChangeEvents::ModifyRowColumnValue(row_column_value) => {
                    if let Some((data_type, value)) =
                        row_column_value.column_values.get(&column_name)
                    {
                        let column_index = table_inserted_data
                            .column_names
                            .iter()
                            .position(|col_name| *col_name == column_name)
                            .unwrap();
                        let original_value = &table_inserted_data.rows[row_index][column_index];
                        if new_value == *original_value {
                            row_column_value.column_values.remove(&column_name);
                            if row_column_value.column_values.len() == 0 {
                                table_data_change_events.remove(event_index);
                            }
                        } else {
                            row_column_value
                                .column_values
                                .insert(column_name, (data_type.clone(), new_value));
                        }
                    } else {
                        row_column_value
                            .column_values
                            .insert(column_name, (data_type, new_value));
                    }
                }
                _ => {} // Handle other enum variants if necessary
            }
        }
    }

    pub fn add_insert_row_event(&self, values: Vec<String>) {
        let locked_table_inserted_data = self.table_inserted_data.blocking_lock();
        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();
        let table_inserted_data = locked_table_inserted_data.as_ref().unwrap();
        locked_table_data_change_events.push(BTableDataChangeEvents::InsertRow(BRowInsertData {
            column_names: table_inserted_data.column_names.clone(),
            values,
            data_types: table_inserted_data.data_types.clone(),
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
        let table_inserted_data = {
            let locked_table_inserted_data = self.table_inserted_data.blocking_lock();
            locked_table_inserted_data.as_ref().unwrap().clone() // Clone to minimize locking duration
        };

        // Step 3: Acquire necessary locks in a consistent order
        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();

        // Step 4: Check if there is an existing row insert event

        if let Some(existing_event_index) = self.find_existing_row_insert_event(
            &locked_table_data_change_events,
            &table_inserted_data,
            row_index,
        ) {
            if let Some(event) = locked_table_data_change_events.get_mut(existing_event_index) {
                self.update_existing_insert_row_event(
                    event,
                    &column_name,
                    &new_value,
                    &table_inserted_data,
                );
                self.console
                    .write(format!("{:?}", locked_table_data_change_events));
            }
            return;
        }

        // Step 2: Check if the row index is in the database
        if row_index >= table_inserted_data.rows.len() {
            return; // Invalid row index, no further processing needed
        }

        let column_datatype_index = table_inserted_data
            .column_names
            .iter()
            .position(|col_name| *col_name == column_name)
            .unwrap();

        let data_type = table_inserted_data.data_types[column_datatype_index].clone();
        // Step 8: Check for existing event and replace if necessary
        if let Some(existing_event_index) = self.find_existing_modify_row_event(
            &locked_table_data_change_events,
            &table_inserted_data,
            row_index,
        ) {
            self.update_modify_row_event(
                &mut locked_table_data_change_events,
                &table_inserted_data,
                row_index,
                existing_event_index,
                column_name,
                new_value,
                data_type,
            );
        } else {
            // Step 7: Proceed with new event creation
            let mut conditions = self.get_primary_key_conditions(row_index, &table_inserted_data);
            let mut column_values = HashMap::new();
            column_values.insert(column_name.clone(), (data_type, new_value.clone()));
            let row_column_value = BRowColumnValue {
                conditions: conditions.clone(),
                column_values,
            };

            // Add the new event
            locked_table_data_change_events.push(BTableDataChangeEvents::ModifyRowColumnValue(
                row_column_value,
            ));
        }
        self.console
            .write(format!("{:?}", locked_table_data_change_events));
    }

    fn update_existing_insert_row_event(
        &self,
        event: &mut BTableDataChangeEvents,
        column_name: &str,
        new_value: &str,
        table_inserted_data: &BTableInsertedData,
    ) {
        match event {
            BTableDataChangeEvents::InsertRow(row_insert_data) => {
                row_insert_data.values =
                    zip(&table_inserted_data.column_names, &row_insert_data.values)
                        .map(|(col_name, value)| {
                            if col_name == column_name {
                                new_value.to_string() // Update the value for the matching column
                            } else {
                                value.to_string() // Keep the existing value
                            }
                        })
                        .collect();
            }
            _ => {} // Handle other enum variants if necessary
        }
    }

    pub fn add_delete_row_event(&self, row_index: usize) {
        // Acquire locks for necessary data
        let locked_table_inserted_data = self.table_inserted_data.blocking_lock();

        let mut locked_table_data_change_events = self.table_data_change_events.blocking_lock();

        // Safely unwrap the locked data
        let table_inserted_data = locked_table_inserted_data.as_ref().unwrap();

        if let Some(existing_event_index) = self.find_existing_row_insert_event(
            &locked_table_data_change_events,
            &table_inserted_data,
            row_index,
        ) {
            locked_table_data_change_events.remove(existing_event_index);
            self.console
                .write(format!("{:?}", locked_table_data_change_events));

            return;
        }
        // Ensure the row index is valid
        else if row_index >= table_inserted_data.rows.len() {
            return; // Exit if the row index is out of bounds
        }

        // Extract conditions based on primary key column names
        let conditions = self.get_primary_key_conditions(row_index, &table_inserted_data);

        // Add the delete row event
        locked_table_data_change_events.push(BTableDataChangeEvents::DeleteRow(conditions));
        let mut locked_current_to_initial_row_indexes =
            self.current_to_initial_row_indexes.blocking_lock();

        let mut keys_to_update: Vec<_> = locked_current_to_initial_row_indexes
            .keys()
            .cloned()
            .filter(|current_row_index| *current_row_index > row_index)
            .collect();

        keys_to_update.sort_by(|a, b| b.cmp(a));

        for (iter_index, current_row_index) in keys_to_update.iter().enumerate() {
            let initial_row_index = locked_current_to_initial_row_indexes
                .get(&current_row_index)
                .unwrap()
                .clone();
            let new_current_row_index = current_row_index - 1;
            locked_current_to_initial_row_indexes
                .insert(new_current_row_index, initial_row_index.clone());
            if iter_index == 0 {
                locked_current_to_initial_row_indexes.remove(current_row_index);
            }
        }
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
            // Use the extracted values without holding the locks
            self.repository
                .update_table_data(&table_name, &table_data_change_events)
                .await;
        }
        self.set_table_data(table_name.to_string()).await;
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
            let table_inserted_data_rows = self
                .repository
                .get_table_data_rows(
                    &table_name,
                    &table_general_info.column_names,
                    &primary_key_column_names,
                )
                .await
                .unwrap();
            let mut locked_current_to_initial_row_indexes =
                self.current_to_initial_row_indexes.lock().await;
            // Construct the inserted data
            let table_inserted_data = BTableInsertedData {
                table_name: table_name.clone(),
                column_names: table_general_info.column_names.clone(),
                data_types: table_general_info.data_types.clone(),
                rows: table_inserted_data_rows
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
            *locked_current_to_initial_row_indexes = HashMap::new();
            for (index, _) in table_inserted_data.rows.iter().enumerate() {
                locked_current_to_initial_row_indexes.insert(index, index);
            } // Update the shared table inserted data
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
    use crate::components::business_components::tables::test_utils::{
        create_btable_general, create_repository_table_and_console, default_table_in, sort_columns,
        sort_tables_general_info,
    };
    use crate::components::business_components::tables::utils::set_tables_general_info;
    use sqlx::PgPool;
    use std::collections::HashMap;

    async fn create_table_data(
        pool: PgPool,
        table_in: &BTableIn,
        insert_row_events: &Vec<BTableDataChangeEvents>,
    ) -> TableData {
        let (repository_result, console_result) =
            create_repository_table_and_console(pool, table_in).await;
        repository_result
            .update_table_data(&table_in.table_name, &insert_row_events)
            .await;
        let tables_general_info = Arc::new(AsyncMutex::new(Vec::<BTableGeneral>::new()));
        set_tables_general_info(repository_result.clone(), tables_general_info.clone()).await;
        let table_data = TableData::new(repository_result, console_result, tables_general_info);
        table_data.set_table_data(table_in.table_name.clone()).await;
        table_data
    }

    #[sqlx::test]
    async fn test_update_table_data(pool: PgPool) {
        let table_in = default_table_in();
        // Define common column names and data types
        // (["id", "name"], [BDataType::INTEGER, BDataType::TEXT])
        let (column_names, data_types): (Vec<String>, Vec<BDataType>) = table_in
            .columns
            .iter()
            .map(|column| (column.name.clone(), column.datatype.clone()))
            .unzip();
        let (id, name) = (column_names[0].clone(), column_names[1].clone());
        // Create a vector of InsertRow events
        let insert_row_events = vec![
            BTableDataChangeEvents::InsertRow(BRowInsertData {
                column_names: column_names.clone(),
                data_types: data_types.clone(),
                values: vec!["1".to_string(), "Alice".to_string()],
            }),
            BTableDataChangeEvents::InsertRow(BRowInsertData {
                column_names: column_names.clone(),
                data_types: data_types.clone(),
                values: vec!["2".to_string(), "Bob".to_string()],
            }),
            BTableDataChangeEvents::InsertRow(BRowInsertData {
                column_names: column_names.clone(),
                data_types: data_types.clone(),
                values: vec!["3".to_string(), "Charlie".to_string()],
            }),
            BTableDataChangeEvents::InsertRow(BRowInsertData {
                column_names: column_names.clone(),
                data_types: data_types.clone(),
                values: vec!["4".to_string(), "Jacob".to_string()],
            }),
        ];
        let table_data = Arc::new(create_table_data(pool, &table_in, &insert_row_events).await);
        let copied_table_data = table_data.clone();
        task::spawn_blocking(move || {
            copied_table_data.add_modify_row_column_value_event(0, id.clone(), "5".to_string());
            copied_table_data.add_modify_row_column_value_event(
                3,
                name.clone(),
                "Liam".to_string(),
            );

            copied_table_data.add_modify_row_column_value_event(3, id.clone(), "8".to_string());
            copied_table_data.add_delete_row_event(1);
            copied_table_data.add_insert_row_event(vec!["6".to_string(), "".to_string()]);
            // row index is out of range
            copied_table_data.add_modify_row_column_value_event(
                4,
                name.clone(),
                "John".to_string(),
            );
            copied_table_data.add_modify_row_column_value_event(
                2,
                name.clone(),
                "Daniel".to_string(),
            );

            // since there was a delete row event index 3 -> 2
        })
        .await;

        table_data.update_table_data().await;
        let expected_table_inserted_data = BTableInsertedData {
            table_name: table_in.table_name.clone(),
            column_names: column_names.clone(),
            data_types: data_types.clone(),
            rows: vec![
                vec!["3".to_string(), "Charlie".to_string()],
                vec!["5".to_string(), "Alice".to_string()],
                vec!["6".to_string(), "".to_string()],
                vec!["8".to_string(), "Daniel".to_string()],
            ],
        };
        let locked_table_inserted_data = table_data.table_inserted_data.lock().await;
        assert_eq!(
            *locked_table_inserted_data,
            Some(expected_table_inserted_data)
        );
    }
}
