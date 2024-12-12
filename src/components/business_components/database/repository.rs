use crate::components::business_components::database::{
    console::RepositoryConsole,
    database::create_database_pool,
    models::{ColumnsInfo, PrimaryKeyConstraint, TableGeneralInfo},
    schemas::{
        ColumnForeignKey, Condition, Constraint, DataType, TableChangeEvents,
        TableDataChangeEvents, TableIn, TableInsertedData,
    },
};
use sqlx::{postgres::PgRow, Executor, PgPool, Postgres, Row, Transaction};
use std::iter::zip;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use tokio::task;

#[derive(Debug, Clone)]
pub struct Repository {
    pool: PgPool,
    console: Arc<RepositoryConsole>,
}

impl Repository {
    pub async fn new(existing_pool: Option<PgPool>, console: Arc<RepositoryConsole>) -> Self {
        if let Some(pool) = existing_pool {
            Self { pool, console }
        } else {
            let pool = create_database_pool().await;
            Self { pool, console }
        }
    }

    async fn log_query(&self, query: String) {
        let console = self.console.clone();
        task::spawn_blocking(move || {
            console.write(query);
        })
        .await;
    }

    pub async fn get_primary_key_column_names(
        &self,
        table_name: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let query = r#"
                            SELECT kcu.column_name
                            FROM information_schema.table_constraints AS tc
                            JOIN information_schema.key_column_usage AS kcu
                            ON tc.constraint_name = kcu.constraint_name
                            AND tc.table_name = kcu.table_name
                            WHERE tc.constraint_type = 'PRIMARY KEY'
                            AND tc.table_name = $1
                         "#;

        let primary_key_column_names: Vec<String> = sqlx::query(query)
            .bind(table_name)
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|row| row.get("column_name"))
            .collect();
        Ok(primary_key_column_names)
    }

    pub async fn get_general_tables_info(&self) -> Result<Vec<TableGeneralInfo>, sqlx::Error> {
        let query = "SELECT
                    t.table_name,
                    array_agg(c.column_name::TEXT) AS column_names,
                    array_agg(c.data_type::TEXT) AS data_types
                FROM
                    information_schema.tables t
                INNER JOIN
                    information_schema.columns c
                ON
                    t.table_name = c.table_name AND t.table_schema = c.table_schema
                WHERE
                    t.table_schema = 'public'
                    AND t.table_type = 'BASE TABLE'
                GROUP BY
                    t.table_name";
        let res = sqlx::query_as::<_, TableGeneralInfo>(query)
            .fetch_all(&self.pool)
            .await;
        res
    }

    pub async fn get_columns_info(
        &self,
        table_name: &str,
    ) -> Result<Vec<ColumnsInfo>, sqlx::Error> {
        let query = "SELECT
                            c.column_name,
                            c.data_type,
                            ARRAY_AGG(tc.constraint_type::TEXT) AS constraint_types,
                            ARRAY_AGG(ccu.table_name::TEXT) AS referenced_tables,
                            ARRAY_AGG(ccu.column_name::TEXT) AS referenced_columns
                        FROM
                            information_schema.columns AS c
                        LEFT JOIN
                            information_schema.key_column_usage AS kcu
                            ON c.table_name = kcu.table_name
                            AND c.column_name = kcu.column_name
                        LEFT JOIN
                            information_schema.table_constraints AS tc
                            ON tc.constraint_name = kcu.constraint_name
                            AND tc.table_name = c.table_name
                        LEFT JOIN
                            information_schema.referential_constraints AS rc
                            ON rc.constraint_name = tc.constraint_name
                        LEFT JOIN
                            information_schema.constraint_column_usage AS ccu
                            ON ccu.constraint_name = rc.unique_constraint_name
                        WHERE
                            c.table_name = $1 
                        GROUP BY c.column_name, c.data_type";
        let parameters = (table_name,);

        let res = sqlx::query_as::<_, ColumnsInfo>(query)
            .bind(parameters.0)
            .fetch_all(&self.pool)
            .await;
        res
    }

    pub async fn get_primary_key_constraint(
        &self,
        table_name: &str,
    ) -> Result<Option<PrimaryKeyConstraint>, sqlx::Error> {
        let query = "SELECT c.conname
                FROM pg_catalog.pg_constraint c
                JOIN pg_class t ON t.oid = c.conrelid
                WHERE t.relname = $1 AND c.contype ='p'";
        let res = sqlx::query_as::<_, PrimaryKeyConstraint>(query)
            .bind(table_name)
            .fetch_optional(&self.pool)
            .await;
        res
    }

    pub async fn create_table(&self, table_in: &TableIn) {
        let mut primary_key_columns = vec![];

        let columns_query_list: Vec<String> = table_in
            .columns
            .iter()
            .map(|column| {
                let mut column_configuration =
                    vec![format!("\"{}\" {}", column.name, column.datatype)];
                for constraint in &column.constraints {
                    match constraint {
                        Constraint::ForeignKey(referenced_table, referenced_column) => {
                            column_configuration.push(format!(
                                "REFERENCES \"{}\"(\"{}\")",
                                referenced_table, referenced_column
                            ));
                        }
                        Constraint::PrimaryKey => {
                            primary_key_columns.push(column.name.clone());
                        }
                    }
                }
                column_configuration.join(" ")
            })
            .collect();

        // If there are primary keys, append the PRIMARY KEY constraint
        let mut full_query_list = columns_query_list.clone();
        if !primary_key_columns.is_empty() {
            full_query_list.push(format!(
                "PRIMARY KEY ({})",
                primary_key_columns
                    .iter()
                    .map(|col| format!("\"{}\"", col))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let columns_query_joined = format!("({})", full_query_list.join(", "));

        // Construct the full SQL query
        let query = format!(
            "CREATE TABLE \"{}\" {}",
            table_in.table_name, columns_query_joined
        );

        // Print the query for debugging
        println!("Generated Query: {}", query);

        // Execute the query
        sqlx::query(&query).execute(&self.pool).await.unwrap();
        self.log_query(query).await;
    }

    pub async fn delete_table(&self, table_name: &str) {
        let query = format!("DROP TABLE \"{}\"", table_name);
        sqlx::query(&query).execute(&self.pool).await.unwrap();
        self.log_query(query).await;
    }

    fn get_filter_condition(&self, conditions: &Vec<Condition>) -> String {
        conditions
            .iter()
            .map(|condition| format!("{} = {}", condition.column_name, condition.value))
            .collect::<Vec<String>>()
            .join(" AND ")
    }

    pub async fn update_table_data(
        &self,
        table_name: &str,
        table_data_change_events: &Vec<TableDataChangeEvents>,
        primary_key_column_names: &Vec<String>,
    ) -> Result<(), sqlx::Error> {
        // Start a transaction
        let mut transaction = self.pool.begin().await?;

        for event in table_data_change_events {
            match event {
                TableDataChangeEvents::ModifyRowColumnValue(row_column_value) => {
                    let filter_condition = self.get_filter_condition(&row_column_value.conditions);
                    // Construct the SQL UPDATE query with dynamic row numbers
                    let bind_parameter = if row_column_value.data_type == DataType::INTEGER {
                        "$1::INTEGER"
                    } else {
                        "$1"
                    };
                    let query = format!(
                        "
                        UPDATE \"{}\"
                        SET \"{}\" = {} 
                        WHERE {}
                       ",
                        table_name,                   // Table for the update
                        row_column_value.column_name, // Column to update
                        bind_parameter,
                        filter_condition
                    );
                    let parameters = (&row_column_value.new_value,);

                    // Execute the query with parameters
                    println!("{}", query);
                    sqlx::query(&query)
                        .bind(parameters.0) // Bind the new value
                        .execute(&mut *transaction)
                        .await
                        .unwrap();
                    self.log_query(query.replace("$1", parameters.0)).await;
                }

                TableDataChangeEvents::DeleteRow(conditions) => {
                    let filter_condition = self.get_filter_condition(&conditions);
                    let query =
                        format!("DELETE FROM \"{}\" WHERE {}", table_name, filter_condition);
                    println!("{}", query);
                    sqlx::query(&query)
                        .execute(&mut *transaction)
                        .await
                        .unwrap();
                    self.log_query(query).await;
                }

                TableDataChangeEvents::InsertRow(row_insert_data) => {
                    let (column_names, values): (Vec<String>, Vec<String>) = row_insert_data
                        .column_names
                        .iter()
                        .zip(
                            row_insert_data
                                .values
                                .iter()
                                .zip(row_insert_data.data_types.iter()),
                        )
                        .map(|(column_name, (value, data_type))| {
                            // Map the filtered columns to (column_name, value) pairs
                            if value.is_empty() && primary_key_column_names.contains(column_name) {
                                // Generate values for primary key columns
                                let generated_value = if *data_type == DataType::INTEGER {
                                    format!(
                                        "(SELECT COUNT(\"{}\") + 1 FROM \"{}\")",
                                        column_name, table_name
                                    )
                                } else if *data_type == DataType::TEXT {
                                    "gen_random_uuid()::TEXT".to_string()
                                } else {
                                    "NULL".to_string() // Fallback for unsupported types
                                };

                                (column_name.to_string(), generated_value)
                            } else {
                                (
                                    column_name.to_string(),
                                    if value.is_empty() {
                                        "NULL".to_string()
                                    } else if *data_type == DataType::TEXT {
                                        format!("'{}'", value)
                                    } else {
                                        value.to_string()
                                    },
                                )
                            }
                        })
                        .unzip();
                    let query = format!(
                        "INSERT INTO \"{}\" ({}) VALUES {}",
                        table_name,
                        column_names.join(", "),
                        format!("({})", values.join(", "))
                    );

                    println!("{}", query);
                    sqlx::query(&query)
                        .execute(&mut *transaction)
                        .await
                        .unwrap();
                    self.log_query(query).await;
                }
            }
        }

        // Commit the transaction
        transaction.commit().await.unwrap();
        Ok(())
    }

    pub async fn get_table_data_rows(
        &self,
        table_name: &str,
        column_names: &Vec<String>,
        order_by_column_names: &Vec<String>,
    ) -> Result<Vec<PgRow>, sqlx::Error> {
        let select_column_names: Vec<String> = column_names
            .into_iter()
            .map(|column_name| {
                format!(
                    "COALESCE(\"{}\"::TEXT, '') AS \"{}\"",
                    column_name, column_name
                )
            })
            .collect();
        let order_by_columns: Vec<String> = order_by_column_names
            .iter()
            .map(|column_name| format!("\"{}\"", column_name))
            .collect();
        let query = format!(
            "SELECT {} FROM \"{}\" ORDER BY {}",
            select_column_names.join(", "),
            table_name,
            order_by_columns.join(", ")
        );
        let table_data_rows = sqlx::query(&query).fetch_all(&self.pool).await;
        table_data_rows
    }

    pub async fn alter_table(
        &self,
        table_name: &str,
        table_change_events: &Vec<TableChangeEvents>,
        initial_primary_key_column_names: &Vec<String>,
    ) -> Result<(), sqlx::Error> {
        // Begin a transaction
        let mut transaction: Transaction<'_, Postgres> = self.pool.begin().await?;
        let mut current_table_name = table_name.to_string();

        let mut primary_key_columns = initial_primary_key_column_names.clone();
        let mut run_drop_primary_constraint_query = true;
        let mut queries = Vec::new();

        for event in table_change_events {
            match event {
                TableChangeEvents::ChangeTableName(new_name) => {
                    queries.push(format!(
                        "ALTER TABLE \"{}\" RENAME TO \"{}\"",
                        current_table_name, new_name
                    ));
                    current_table_name = new_name.clone();
                }
                TableChangeEvents::ChangeColumnDataType(column_name, new_data_type) => {
                    queries.push(format!(
                        "ALTER TABLE \"{}\" ALTER COLUMN \"{}\" TYPE {} USING \"{}\"::{}",
                        current_table_name, column_name, new_data_type, column_name, new_data_type
                    ));
                }
                TableChangeEvents::ChangeColumnName(old_name, new_name) => {
                    queries.push(format!(
                        "ALTER TABLE \"{}\" RENAME COLUMN \"{}\" TO \"{}\"",
                        current_table_name, old_name, new_name
                    ));
                }
                TableChangeEvents::AddColumn(column_name, data_type) => {
                    queries.push(format!(
                        "ALTER TABLE \"{}\" ADD COLUMN \"{}\" {}",
                        current_table_name, column_name, data_type
                    ));
                }
                TableChangeEvents::RemoveColumn(column_name) => {
                    if let Some(existing_index) = primary_key_columns
                        .iter()
                        .position(|primary_key_column_name| primary_key_column_name == column_name)
                    {
                        // implicitly runs drop constraint query if primary key column is dropped
                        run_drop_primary_constraint_query = false;
                        primary_key_columns.remove(existing_index);
                    }
                    queries.push(format!(
                        "ALTER TABLE \"{}\" DROP COLUMN \"{}\"",
                        current_table_name, column_name
                    ));
                }
                TableChangeEvents::AddForeignKey(column_foreign_key) => {
                    queries.push(format!(
                    "ALTER TABLE \"{}\" ADD CONSTRAINT fk_{}_{} FOREIGN KEY (\"{}\") REFERENCES \"{}\" (\"{}\")",
                    current_table_name, current_table_name, column_foreign_key.column_name,
                    column_foreign_key.column_name, column_foreign_key.referenced_table,
                    column_foreign_key.referenced_column
                ));
                }
                TableChangeEvents::RemoveForeignKey(column_name) => {
                    queries.push(format!(
                        "ALTER TABLE \"{}\" DROP CONSTRAINT IF EXISTS fk_{}_{}",
                        current_table_name, current_table_name, column_name,
                    ));
                }
                TableChangeEvents::AddPrimaryKey(column_name) => {
                    primary_key_columns.push(column_name.clone());
                }
                TableChangeEvents::RemovePrimaryKey(column_name) => {
                    if let Some(existing_index) = primary_key_columns
                        .iter()
                        .position(|existing_column_name| existing_column_name == column_name)
                    {
                        primary_key_columns.remove(existing_index);
                    }
                }
            }
        }

        // Handle primary key changes separately
        if *initial_primary_key_column_names != primary_key_columns {
            if run_drop_primary_constraint_query {
                if let Some(primary_key_constraint) =
                    self.get_primary_key_constraint(&table_name).await.unwrap()
                {
                    let drop_query = format!(
                        "ALTER TABLE \"{}\" DROP CONSTRAINT \"{}\"",
                        current_table_name, primary_key_constraint.conname
                    );
                    queries.push(drop_query);
                }
            }
            if !primary_key_columns.is_empty() {
                let add_query = format!(
                    "ALTER TABLE \"{}\" ADD CONSTRAINT pk_{} PRIMARY KEY ({})",
                    current_table_name,
                    current_table_name,
                    primary_key_columns.join(", ")
                );
                queries.push(add_query);
            }
        }

        // Execute each query in the transaction
        for query in queries {
            println!("{}", query);
            sqlx::query(&query).execute(&mut *transaction).await?;
            self.log_query(query).await;
        }

        // Commit the transaction
        transaction.commit().await?;

        Ok(())
    }
}
