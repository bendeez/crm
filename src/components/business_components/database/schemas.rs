use crate::components::business_components::database::models::{ColumnsInfo, TableGeneralInfo};
use std::collections::HashMap;
use std::fmt;
use std::iter::zip;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataType {
    TEXT,
    INTEGER,
    TIMESTAMP,
    SERIAL,
    BOOLEAN,
}

impl Default for DataType {
    fn default() -> Self {
        DataType::TEXT
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            DataType::TEXT => write!(f, "TEXT"),
            DataType::INTEGER => write!(f, "INTEGER"),
            DataType::TIMESTAMP => write!(f, "TIMESTAMP"),
            DataType::SERIAL => write!(f, "SERIAL"),
            DataType::BOOLEAN => write!(f, "BOOLEAN"),
        }
    }
}

impl DataType {
    pub fn to_datatype(value: String) -> Self {
        match value.as_str() {
            "text" => Self::TEXT,
            "integer" => Self::INTEGER,
            "timestamp without time zone" => Self::TIMESTAMP,
            "serial" => Self::SERIAL,
            "boolean" => Self::BOOLEAN,
            _ => panic!("Invalid datatype"),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    ForeignKey(String, String),
    PrimaryKey,
}

impl fmt::Display for Constraint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Constraint::ForeignKey(referenced_table, referenced_column) => {
                write!(f, "REFERENCES {}({})", referenced_table, referenced_column)
            }
            Constraint::PrimaryKey => write!(f, "PRIMARY KEY"),
        }
    }
}

impl Constraint {
    pub fn to_constraint(
        constraint_type: String,
        referenced_table: String,
        referenced_column: String,
    ) -> Self {
        match constraint_type.as_str() {
            "PRIMARY KEY" => Self::PrimaryKey,
            "FOREIGN KEY" => Self::ForeignKey(referenced_table, referenced_column),
            _ => panic!("Invalid Constraint"),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct Column {
    pub name: String,
    pub datatype: DataType,
    pub constraints: Vec<Constraint>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct TableGeneral {
    pub table_name: String,
    pub column_names: Vec<String>,
    pub data_types: Vec<DataType>,
}

impl TableGeneral {
    pub fn to_table(table_general_info: TableGeneralInfo) -> Self {
        Self {
            table_name: table_general_info.table_name,
            column_names: table_general_info.column_names,
            data_types: table_general_info
                .data_types
                .into_iter()
                .map(|data_type| DataType::to_datatype(data_type))
                .collect(),
        }
    }
}

impl Column {
    pub fn to_column(column_info: ColumnsInfo) -> Self {
        // initial query couldve returned null constraint types so they
        // need to be filtered
        Self {
            name: column_info.column_name,
            datatype: DataType::to_datatype(column_info.data_type),
            constraints: zip(
                zip(column_info.constraint_types, column_info.referenced_tables),
                column_info.referenced_columns,
            )
            .filter(|((constraint_type, referenced_table), referenced_column)| {
                !constraint_type.is_none()
            })
            .map(|((constraint_type, referenced_table), referenced_column)| {
                Constraint::to_constraint(
                    constraint_type.unwrap(),
                    referenced_table.unwrap_or_default(),
                    referenced_column.unwrap_or_default(),
                )
            })
            .collect(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct TableIn {
    pub table_name: String,
    pub columns: Vec<Column>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnForeignKey {
    pub column_name: String,
    pub referenced_column: String,
    pub referenced_table: String,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct TableInsertedData {
    pub table_name: String,
    pub column_names: Vec<String>,
    pub data_types: Vec<DataType>,
    pub rows: Vec<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableChangeEvents {
    ChangeTableName(String),
    ChangeColumnDataType(String, DataType),
    ChangeColumnName(String, String),
    AddColumn(String, DataType),
    RemoveColumn(String),
    AddForeignKey(ColumnForeignKey),
    RemoveForeignKey(String),
    AddPrimaryKey(String),
    RemovePrimaryKey(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Condition {
    pub column_name: String,
    pub data_type: DataType,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowColumnValue {
    pub conditions: Vec<Condition>,
    pub column_name: String,
    pub data_type: DataType,
    pub new_value: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowInsertData {
    pub column_names: Vec<String>,
    pub data_types: Vec<DataType>,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TableDataChangeEvents {
    ModifyRowColumnValue(RowColumnValue),
    DeleteRow(Vec<Condition>),
    InsertRow(RowInsertData),
}
