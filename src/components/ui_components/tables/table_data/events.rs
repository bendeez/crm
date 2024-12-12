use crate::components::business_components::{
    component::{BDataType, BTableChangeEvents, BTableGeneral, BTableIn, BTableInsertedData},
    components::BusinessTables,
};
use crate::components::ui_components::{
    component::Event, events::Message, tables::events::TablesMessage,
};

#[derive(Debug, Clone)]
pub enum TableDataMessage {
    GetTableData(String),
    SetTableData,
    UpdateCell(usize, usize, String),
    DeleteRow(usize),
    AddRow,
    UpdateTableData,
}

impl Event for TableDataMessage {
    fn message(self) -> Message {
        TablesMessage::SingleTableData(self).message()
    }
}
