use crate::components::business_components::{
    component::{BColumn, BConstraint, BDataType, BTableGeneral, BTableIn, BusinessComponent},
    components::BusinessTables,
};
use crate::components::ui_components::{
    component::{Event, UIComponent},
    events::Message,
    tables::{
        create_table_form::CreateTableFormUI,
        events::{CreateTableFormMessage, TablesMessage},
        table_data::table_data::TableDataUI,
        table_info::table_info::TableInfoUI,
    },
};
use iced::{
    alignment,
    alignment::{Alignment, Vertical},
    border::Radius,
    futures::join,
    widget::{
        button, checkbox, column, container, row, scrollable, text, text_input, Button, Checkbox,
        Column, PickList, Row, Text,
    },
    Background, Border, Color, Element, Length, Shadow, Task, Theme, Vector,
};
use regex::Regex;
use std::iter::zip;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Clone)]
pub struct TablesUI {
    pub table_filter: String,
    pub show_create_table_form: bool,
    pub create_table_form: CreateTableFormUI,
    pub tables: Arc<BusinessTables>,
    pub single_table_info: Option<TableInfoUI>,
    pub show_single_table_data: bool,
    pub single_table_data: TableDataUI,
    pub table_to_delete: Option<String>,
}

impl UIComponent for TablesUI {
    type EventType = TablesMessage;

    fn update(&mut self, message: Self::EventType) -> Task<Message> {
        match message {
            Self::EventType::UpdateTableFilter(input) => {
                self.table_filter = input;
                Task::none()
            }
            Self::EventType::ShowOrRemoveCreateTableForm => {
                self.show_create_table_form = !self.show_create_table_form;
                self.create_table_form
                    .update(CreateTableFormMessage::ShowOrRemoveCreateTableForm)
            }
            Self::EventType::ShowOrRemoveTableData => {
                if self.show_single_table_data {
                    self.show_single_table_data = false;
                } else {
                    self.show_single_table_data = true;
                }
                Task::none()
            }
            Self::EventType::CreateTableForm(create_table_form_message) => {
                match &create_table_form_message {
                    CreateTableFormMessage::TableCreated(table_name) => {
                        let task_result = self
                            .create_table_form
                            .update(create_table_form_message.clone());
                        self.show_create_table_form = false;
                        task_result.chain(Task::done(Self::EventType::message(
                            Self::EventType::GetSingleTableInfo(table_name.clone()),
                        )))
                    }
                    _ => self.create_table_form.update(create_table_form_message),
                }
            }
            Self::EventType::GetSingleTableInfo(table_name) => {
                let tables = self.tables.clone();

                Task::perform(
                    async move {
                        tables.table_info.set_table_info(table_name).await;
                    },
                    |_| Self::EventType::SetSingleTableInfo.message(),
                )
            }
            Self::EventType::SetSingleTableInfo => {
                self.single_table_info = Some(TableInfoUI::new(self.tables.table_info.clone()));
                Task::none()
            }
            Self::EventType::UndisplayTableInfo => {
                self.single_table_info = None;
                Task::none()
            }
            Self::EventType::SingleTableInfo(table_info_message) => {
                if let Some(table_info) = &mut self.single_table_info {
                    table_info.update(table_info_message)
                } else {
                    Task::none()
                }
            }
            Self::EventType::SingleTableData(table_data_message) => {
                self.single_table_data.update(table_data_message)
            }

            Self::EventType::RequestDeleteTable(table_name) => {
                self.table_to_delete = Some(table_name);
                Task::none()
            }
            Self::EventType::InitializeComponent => {
                let tables = self.tables.clone();
                Task::perform(
                    async move {
                        tables.initialize_component().await;
                    },
                    |_| Self::EventType::ComponentInitialized.message(),
                )
            }
            Self::EventType::ComponentInitialized => {
                Task::done(Self::EventType::SetTables.message())
            }
            Self::EventType::ConfirmDeleteTable => {
                if let Some(table_to_delete) = self.table_to_delete.clone() {
                    if let Some(single_table_info) = &self.single_table_info {
                        if single_table_info.get_table_name() == table_to_delete {
                            self.single_table_info = None;
                        }
                    }
                    self.table_to_delete = None;
                    let tables = self.tables.clone();

                    Task::perform(
                        async move {
                            tables.delete_table(table_to_delete).await;
                        },
                        |_| Self::EventType::SetTables.message(),
                    )
                } else {
                    Task::none()
                }
            }
            Self::EventType::CancelDeleteTable => {
                self.table_to_delete = None;
                Task::none()
            }
            Self::EventType::SetTables => Task::none(),
        }
    }
}

impl TablesUI {
    pub fn new(tables: Arc<BusinessTables>) -> Self {
        Self {
            table_filter: String::default(),
            show_create_table_form: false,
            show_single_table_data: false,
            create_table_form: CreateTableFormUI::new(tables.clone()),
            single_table_data: TableDataUI::new(tables.table_data.clone()),
            tables,
            single_table_info: None,
            table_to_delete: None,
        }
    }

    pub fn content<'a>(&'a self) -> Element<'a, Message> {
        let mut row = Row::new()
            .height(Length::Fill)
            .width(Length::Fill)
            .spacing(20)
            .padding(20);

        row = row.push(self.tables_section());
        if self.show_create_table_form {
            row = row.push(self.create_table_form.content());
        }

        // Display single table info with an "Undisplay" button
        if let Some(table_info) = &self.single_table_info {
            let mut table_info_section = Column::new().spacing(10).padding(10);
            table_info_section = table_info_section.push(table_info.content());

            let undisplay_button = button("🔙 Back")
                .style(|_, _| button_style())
                .on_press(<TablesUI as UIComponent>::EventType::UndisplayTableInfo.message())
                .padding(10);

            table_info_section = table_info_section.push(undisplay_button);

            row = row.push(container(table_info_section).width(Length::Fill));
        }

        if self.show_single_table_data {
            row = row.push(container(self.single_table_data.content()).width(Length::Fill));
        }

        container(row)
            .height(Length::Fill)
            .width(Length::Fill)
            .padding(20)
            .style(|_| container_style())
            .into()
    }

    // ======================== SECTION: Tables Display ========================

    fn tables_section<'a>(&'a self) -> Element<'a, Message> {
        let mut tables_display = Column::new().spacing(10).padding(10);
        tables_display = tables_display.push(self.table_filter_input());
        tables_display = tables_display.push(self.tables_container());

        let scrollable_section = scrollable(
            container(tables_display)
                .padding(10)
                .style(|_| container_style()),
        )
        .height(Length::Fill)
        .width(Length::Fill);

        let toggle_form_button = button(if self.show_create_table_form {
            "Remove create table form"
        } else {
            "Show create table form"
        })
        .style(|_, _| button_style())
        .on_press(<TablesUI as UIComponent>::EventType::ShowOrRemoveCreateTableForm.message())
        .padding(10);

        let toggle_table_data_button = button(if self.show_single_table_data {
            "Remove table data"
        } else {
            "Show table_data"
        })
        .style(|_, _| button_style())
        .on_press(<TablesUI as UIComponent>::EventType::ShowOrRemoveTableData.message())
        .padding(10);

        Column::new()
            .push(scrollable_section)
            .push(toggle_form_button)
            .push(toggle_table_data_button)
            .spacing(10)
            .padding(10)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn table_filter_input<'a>(&'a self) -> Element<'a, Message> {
        text_input("Search Tables", &self.table_filter)
            .on_input(|input| {
                <TablesUI as UIComponent>::EventType::message(
                    <TablesUI as UIComponent>::EventType::UpdateTableFilter(input),
                )
            })
            .width(Length::Fill)
            .padding(10)
            .style(|_, _| text_input_style())
            .into()
    }
    fn delete_table_styled_confirmation_text<'a>(&'a self) -> Element<'a, Message> {
        let message_prefix = Text::new("Are you sure you want to delete the table ")
            .size(20)
            .color(Color::from_rgb(0.9, 0.9, 0.9)); // Light grey color for the main text

        let highlighted_table_name = text(self.table_to_delete.as_ref().unwrap())
            .size(22)
            .color(Color::from_rgb(1.0, 0.4, 0.4)); // Emphasized red color for the table name

        let message_suffix = Text::new("?")
            .size(20)
            .color(Color::from_rgb(0.9, 0.9, 0.9));

        // Combine the styled texts into a row
        Row::new()
            .push(message_prefix)
            .push(highlighted_table_name)
            .push(message_suffix)
            .align_y(Vertical::Center)
            .wrap()
            .into()
    }
    fn delete_table_confirmation_modal<'a>(&'a self) -> Element<'a, Message> {
        let confirm_button = Button::new(text("Yes, delete"))
            .on_press(<TablesUI as UIComponent>::EventType::ConfirmDeleteTable.message())
            .style(|_, _| delete_button_style());

        let cancel_button = Button::new(text("Cancel"))
            .on_press(<TablesUI as UIComponent>::EventType::CancelDeleteTable.message());

        let modal_content = container(
            Column::new()
                .spacing(20)
                .push(self.delete_table_styled_confirmation_text())
                .push(
                    Row::new()
                        .spacing(10)
                        .push(confirm_button)
                        .push(cancel_button),
                ),
        )
        .padding(20)
        .style(|_| delete_table_confirmation_modal_style());

        container(modal_content).padding(20).into()
    }
    fn tables_container<'a>(&'a self) -> Element<'a, Message> {
        let locked_tables_general_info = self.tables.tables_general_info.blocking_lock();
        let mut tables_column = Column::new().spacing(10).padding(10);
        let table_filter_pattern = self.get_table_filter_regex();

        for table in locked_tables_general_info
            .clone()
            .into_iter()
            .filter(|t| table_filter_pattern.is_match(&t.table_name))
        {
            let view_button = button(text(table.table_name.clone())).on_press(
                <TablesUI as UIComponent>::EventType::message(
                    <TablesUI as UIComponent>::EventType::GetSingleTableInfo(
                        table.table_name.clone(),
                    ),
                ),
            );

            let delete_button = button(text("🗑️ Delete"))
                .style(|_, _| delete_button_style())
                .on_press(<TablesUI as UIComponent>::EventType::message(
                    <TablesUI as UIComponent>::EventType::RequestDeleteTable(
                        table.table_name.clone(),
                    ),
                ));

            let table_row = Row::new().spacing(10).push(view_button).push(delete_button);

            tables_column = tables_column.push(table_row);
        }

        if !self.table_to_delete.is_none() {
            return self.delete_table_confirmation_modal();
        }

        tables_column.into()
    } // ======================== SECTION: Create Table ========================

    fn get_table_filter_regex(&self) -> Regex {
        Regex::new(&format!(r"(?i){}", self.table_filter))
            .unwrap_or_else(|_| Regex::new("").unwrap())
    }
}

// ======================== STYLES ========================
fn container_style() -> container::Style {
    container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))), // Background color
        border: Border {
            color: Color::TRANSPARENT,
            width: 1.5,
            radius: Radius::from(5.0),
        },
        text_color: Some(Color::WHITE), // Text color for the content inside the container
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 2.0),
            blur_radius: 5.0,
        },
    }
}
fn button_style() -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb(0.0, 0.75, 0.65))),
        border: Border {
            color: Color::from_rgb(0.0, 0.6, 0.5),
            width: 2.0,
            radius: Radius::from(5.0),
        },
        text_color: Color::WHITE,
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 5.0,
        },
    }
}

fn delete_button_style() -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb(0.8, 0.2, 0.2))), // Soft red background
        border: Border {
            color: Color::from_rgb(0.6, 0.1, 0.1), // Dark red border
            width: 2.0,
            radius: Radius::from(5.0),
        },
        text_color: Color::WHITE, // White text for contrast
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 5.0,
        },
    }
}

fn create_button_style() -> button::Style {
    button::Style {
        background: Some(Background::Color(Color::from_rgb(0.0, 0.5, 0.9))),
        border: Border {
            color: Color::from_rgb(0.0, 0.4, 0.7),
            width: 2.0,
            radius: Radius::from(8.0),
        },
        text_color: Color::WHITE,
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 3.0),
            blur_radius: 7.0,
        },
    }
}

fn text_input_style() -> text_input::Style {
    text_input::Style {
        background: Background::Color(Color::from_rgb(0.2, 0.2, 0.2)), // Darker input background
        border: Border {
            width: 1.5,
            color: Color::from_rgb(0.0, 0.74, 0.84),
            radius: Radius::from(5.0),
        },
        placeholder: Color::from_rgb(0.6, 0.6, 0.6), // Color for placeholder text
        value: Color::WHITE,                         // Color for input text
        selection: Color::from_rgb(0.0, 0.74, 0.84), // Color for selected text
        icon: Color::from_rgb(0.8, 0.8, 0.8),        // Color for any input icons
    }
}

fn delete_table_confirmation_modal_style() -> container::Style {
    container::Style {
        // Semi-transparent dark background
        background: Some(Background::Color(Color::from_rgba(0.05, 0.05, 0.05, 0.95))),

        // Softer border with a slightly transparent white color
        border: Border {
            color: Color::from_rgba(1.0, 1.0, 1.0, 0.3),
            width: 1.0,
            radius: Radius::from(12.0),
        },

        // White text color for readability
        text_color: Some(Color::WHITE),

        // Softer shadow for a subtle floating effect
        shadow: Shadow {
            color: Color::from_rgba(0.0, 0.0, 0.0, 0.7),
            offset: Vector::new(0.0, 5.0),
            blur_radius: 15.0,
        },
    }
}
