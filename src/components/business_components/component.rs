use crate::components::business_components::database::models::ColumnsInfo;
use crate::components::business_components::database::schemas::{
    Column, ColumnForeignKey, Condition, Constraint, DataType, RowColumnValue, RowInsertData,
    TableChangeEvents, TableDataChangeEvents, TableGeneral, TableIn, TableInsertedData,
};
use crate::components::business_components::tables::{
    table_data::table_data::TableData, table_info::table_info::TableInfo,
};

pub type BColumn = Column;
pub type BDataType = DataType;
pub type BTableIn = TableIn;
pub type BTableChangeEvents = TableChangeEvents;
pub type BTableDataChangeEvents = TableDataChangeEvents;
pub type BTableInfo = TableInfo;
pub type BTableData = TableData;
pub type BTableGeneral = TableGeneral;
pub type BConstraint = Constraint;
pub type BColumnForeignKey = ColumnForeignKey;
pub type BCondition = Condition;
pub type BTableInsertedData = TableInsertedData;
pub type BRowColumnValue = RowColumnValue;
pub type BRowInsertData = RowInsertData;

pub trait BusinessComponent {
    async fn initialize_component(&self) {}
}

pub(super) mod repository_module {
    use crate::components::business_components::database::console::RepositoryConsole;
    use crate::components::business_components::database::repository::Repository;

    pub type BRepository = Repository;
    pub type BRepositoryConsole = RepositoryConsole;
}
