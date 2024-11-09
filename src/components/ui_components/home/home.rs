use crate::components::business_components::{
    component::{BColumn, BDataType, BTable, BTableIn, BusinessComponent},
    components::BusinessHome,
};
use crate::components::ui_components::{
    component::UIComponent, events::Message, home::events::HomeMessage,
};
use iced::{
    widget::{
        button, column, container, row, scrollable, text, text_input, Column, PickList, Row, Text,
    },
    Alignment, Element, Length, Task,
};
use regex::Regex;

#[derive(Debug, Clone)]
pub struct HomeUI {
    pub home: BusinessHome,
    pub table_filter: String,
    pub show_create_table_form: bool,
    pub create_table_input: BTableIn, // New field to store columns
}

#[derive(Debug, Clone)]
pub enum ColumnMessage {
    NameChanged(usize, String),
    DatatypeChanged(usize, String),
    AddColumn,
    RemoveColumn(usize),
}

impl UIComponent for HomeUI {
    type EventType = HomeMessage;

    async fn initialize_component(&mut self) {
        self.home.initialize_component().await;
    }

    fn update(&mut self, message: Self::EventType) -> Task<Message> {
        match message {
            Self::EventType::InitializeComponent => {
                let mut home_ui = self.clone();
                Task::perform(
                    async move {
                        home_ui.initialize_component().await;
                        home_ui
                    },
                    |home_ui_initialized| {
                        Message::Home(Self::EventType::ComponentUpdated(home_ui_initialized))
                    },
                )
            }
            Self::EventType::ComponentUpdated(home_ui_updated) => {
                *self = home_ui_updated;
                Task::none()
            }
            Self::EventType::UpdateTableFilter(input) => {
                self.table_filter = input;
                Task::none()
            }
            Self::EventType::ShowCreateTableForm => {
                self.show_create_table_form = !self.show_create_table_form;
                Task::none()
            }
            Self::EventType::AddColumn => {
                self.create_table_input.columns.push(BColumn::default());
                Task::none()
            }
            Self::EventType::RemoveColumn(index) => {
                if index < self.create_table_input.columns.len() {
                    self.create_table_input.columns.remove(index);
                }
                Task::none()
            }
            Self::EventType::UpdateColumnName(index, input) => {
                if let Some(column) = self.create_table_input.columns.get_mut(index) {
                    column.name = input;
                }
                Task::none()
            }
            Self::EventType::UpdateColumnType(index, input) => {
                if let Some(column) = self.create_table_input.columns.get_mut(index) {
                    column.datatype = input;
                }
                Task::none()
            }
            Self::EventType::UpdateTableName(input) => {
                self.create_table_input.table_name = input;
                Task::none()
            }
            Self::EventType::SubmitCreateTable => {
                let mut home_ui = self.clone();
                let create_table_input = self.create_table_input.clone();
                self.create_table_input = BTableIn::default();
                self.show_create_table_form = false;
                Task::perform(
                    async move {
                        home_ui.home.add_table(create_table_input).await;
                        home_ui
                    },
                    |home_ui_updated| {
                        Message::Home(Self::EventType::ComponentUpdated(home_ui_updated))
                    },
                )
            }
        }
    }
}

impl HomeUI {
    pub fn new(home: BusinessHome) -> Self {
        Self {
            home,
            table_filter: String::new(),
            show_create_table_form: false,
            create_table_input: BTableIn::default(),
        }
    }

    fn create_table_form<'a>(&'a self) -> Element<'a, Message> {
        let mut form = Column::new()
            .height(Length::Fill)
            .width(Length::Fill)
            .padding(10)
            .spacing(10);
        let table_name_input = text_input("Table Name", &self.create_table_input.table_name)
            .on_input(move |value| Message::Home(HomeMessage::UpdateTableName(value)))
            .width(400);
        form = form.push(row![table_name_input]);
        // Iterate over existing columns and create input fields for each
        for (index, column) in self.create_table_input.columns.iter().enumerate() {
            let name_input = text_input("Column Name", &column.name)
                .on_input(move |value| Message::Home(HomeMessage::UpdateColumnName(index, value)))
                .width(200);

            // Use a PickList for the data type dropdown
            let datatype_input = PickList::new(
                vec![BDataType::TEXT, BDataType::INT, BDataType::DATETIME],
                Some(&column.datatype),
                move |value| Message::Home(HomeMessage::UpdateColumnType(index, value)),
            )
            .placeholder("Data Type")
            .width(200);

            let remove_button = button("Remove")
                .on_press(Message::Home(HomeMessage::RemoveColumn(index)))
                .padding(5);

            form = form.push(row![name_input, datatype_input, remove_button].spacing(10));
        }

        // Add button to add new columns
        let add_column_button = button("Add Column")
            .on_press(Message::Home(HomeMessage::AddColumn))
            .padding(10);

        form = form.push(add_column_button);

        let create_table_button = button("Create table")
            .on_press(Message::Home(HomeMessage::SubmitCreateTable))
            .padding(10);
        form = form.push(row![create_table_button]);
        container(form).into()
    }

    fn tables<'a>(&'a self) -> Element<'a, Message> {
        let tables_container = if let Some(tables) = &self.home.tables {
            let mut tables_column = Column::new()
                .height(Length::Fill)
                .width(Length::Fill)
                .padding(10);

            let table_filter_pattern = Regex::new(&format!(r"(?i){}", &self.table_filter))
                .unwrap_or_else(|error| {
                    eprintln!("{}", error);
                    Regex::new(r"").unwrap()
                });

            let tables_filtered: Vec<_> = tables
                .into_iter()
                .filter(|table| table_filter_pattern.is_match(&table.table_name))
                .collect();

            for table in tables_filtered {
                tables_column = tables_column.push(text(&table.table_name));
            }
            container(tables_column).height(250).width(300)
        } else {
            container(text("Loading"))
                .height(Length::Fill)
                .width(Length::Fill)
                .padding(10)
        };

        let text_input = text_input("Search", &self.table_filter)
            .on_input(|input| Message::Home(HomeMessage::UpdateTableFilter(input)))
            .width(300);

        let mut tables_display = Column::new();
        tables_display = tables_display.push(tables_container);
        tables_display = tables_display.push(text_input);
        let show_create_table_form_button = button(if self.show_create_table_form {
            "Remove create table form"
        } else {
            "Show create table form"
        })
        .on_press(Message::Home(HomeMessage::ShowCreateTableForm));
        tables_display = tables_display.push(show_create_table_form_button);

        // Conditionally show the form
        if self.show_create_table_form {
            tables_display = tables_display.push(self.create_table_form());
        }

        container(tables_display).into()
    }

    pub fn content<'a>(&'a self) -> Element<'a, Message> {
        let mut row = Row::new();
        row = row.push(self.tables());
        row = row.push(self.title());
        container(row).into()
    }

    fn title<'a>(&'a self) -> Element<'a, Message> {
        if let Some(title) = &self.home.title {
            container(text(title)).into()
        } else {
            container(text("Loading")).into()
        }
    }
}
