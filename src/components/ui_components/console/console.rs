use crate::components::business_components::components::BusinessConsole;
use crate::components::ui_components::component::{Event, UIComponent};
use crate::components::ui_components::console::events::ConsoleMessage;
use crate::components::ui_components::events::Message;
use iced::{
    border,
    border::Radius,
    font::Font,
    widget::{
        button, column, container, row, scrollable, text, text_input, Column, Container, PickList,
        Row, Scrollable, Text, TextInput,
    },
    Alignment, Background, Border, Color, Element, Length, Shadow, Task, Theme, Vector,
};
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;

#[derive(Debug, Clone, PartialEq)]
pub enum SelectedConsole {
    Database,
    Business,
    UI,
}

#[derive(Debug, Clone)]
pub struct ConsoleUI {
    console: Arc<BusinessConsole>,
    messages: Vec<String>,
    selected_console: SelectedConsole, // Track the selected tab
}

impl UIComponent for ConsoleUI {
    type EventType = ConsoleMessage;

    fn update(&mut self, message: Self::EventType) -> Task<Message> {
        match message {
            Self::EventType::LogMessage(message) => {
                self.messages.push(message);
                Task::none()
            }
            Self::EventType::ClearMessages(selected_console) => {
                match selected_console {
                    SelectedConsole::UI => {
                        self.messages = vec![];
                    }
                    SelectedConsole::Business => {
                        self.console.clear_messages();
                    }
                    SelectedConsole::Database => {
                        self.console.clear_database_messages();
                    }
                }
                Task::none()
            }
            Self::EventType::SwitchTab(selected_console) => {
                self.selected_console = selected_console;
                Task::none()
            }
        }
    }
}

impl ConsoleUI {
    pub fn new(console: Arc<BusinessConsole>) -> Self {
        Self {
            messages: vec![],
            console,
            selected_console: SelectedConsole::UI,
        }
    }

    pub fn content(&self) -> Column<'_, Message> {
        let mut console_display = Column::new().spacing(10).padding(10);

        // Select which messages to display based on the selected console tab
        match self.selected_console {
            SelectedConsole::UI => {
                for message in &self.messages {
                    let text_widget = Text::new(message)
                        .size(16)
                        .width(Length::Fill)
                        .color(Color::from_rgb(0.8, 0.8, 0.8));
                    let message_container = Container::new(text_widget)
                        .padding(10)
                        .width(Length::Fill)
                        .style(|_| console_message_style());
                    console_display = console_display.push(message_container);
                }
            }
            SelectedConsole::Business => {
                // Display business console messages
                for message in self.console.get_messages() {
                    let text_widget = Text::new(message)
                        .size(16)
                        .width(Length::Fill)
                        .color(Color::from_rgb(0.8, 0.8, 0.8));
                    let message_container = Container::new(text_widget)
                        .padding(10)
                        .width(Length::Fill)
                        .style(|_| console_message_style());
                    console_display = console_display.push(message_container);
                }
            }
            SelectedConsole::Database => {
                // Display database messages
                for message in self.console.get_database_messages() {
                    let text_widget = Text::new(message)
                        .size(16)
                        .width(Length::Fill)
                        .color(Color::from_rgb(0.8, 0.8, 0.8));
                    let message_container = Container::new(text_widget)
                        .padding(10)
                        .width(Length::Fill)
                        .style(|_| console_message_style());
                    console_display = console_display.push(message_container);
                }
            }
        }

        // Wrap the messages in a scrollable container
        let scrollable_console = scrollable(
            container(console_display)
                .style(|_| console_style())
                .padding(10),
        )
        .height(Length::Fill)
        .width(400)
        .style(|_, _| scrollbar_style());

        // Tab switch buttons with styling
        let ui_button = button(Text::new("UI Messages"))
            .style(|_, _| button_style(self.selected_console == SelectedConsole::UI))
            .on_press(
                <ConsoleUI as UIComponent>::EventType::SwitchTab(SelectedConsole::UI).message(),
            );

        let business_button = button(Text::new("Business Messages"))
            .style(|_, _| button_style(self.selected_console == SelectedConsole::Business))
            .on_press(
                <ConsoleUI as UIComponent>::EventType::SwitchTab(SelectedConsole::Business)
                    .message(),
            );

        let database_button = button(Text::new("Database Messages"))
            .style(|_, _| button_style(self.selected_console == SelectedConsole::Database))
            .on_press(
                <ConsoleUI as UIComponent>::EventType::SwitchTab(SelectedConsole::Database)
                    .message(),
            );

        // Conditionally add the clear buttons for each tab
        let clear_button = match self.selected_console {
            SelectedConsole::UI => button(Text::new("Clear UI Messages")).padding(10).on_press(
                <ConsoleUI as UIComponent>::EventType::ClearMessages(SelectedConsole::UI).message(),
            ),
            SelectedConsole::Business => button(Text::new("Clear Business Messages"))
                .padding(10)
                .on_press(
                    <ConsoleUI as UIComponent>::EventType::ClearMessages(SelectedConsole::Business)
                        .message(),
                ),
            SelectedConsole::Database => button(Text::new("Clear Database Messages"))
                .padding(10)
                .on_press(
                    <ConsoleUI as UIComponent>::EventType::ClearMessages(SelectedConsole::Database)
                        .message(),
                ),
        };

        // Start building the column layout
        let mut column = Column::new().spacing(10).push(
            Row::new()
                .spacing(10)
                .push(ui_button)
                .push(business_button)
                .push(database_button),
        ); // Row for tab buttons

        column = column.push(scrollable_console);

        // Add the clear button if it exists
        column = column.push(clear_button);

        column
    }
}

// ======================== STYLES ========================

// Style for the individual console messages
fn console_message_style() -> container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb(0.15, 0.15, 0.15))),
        text_color: Some(Color::from_rgb(0.8, 0.8, 0.8)), // Light gray text
        border: Border {
            color: Color::from_rgb(0.4, 0.4, 0.4),
            width: 1.0,
            radius: Radius::from(5.0),
        },
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 2.0),
            blur_radius: 3.0,
        },
    }
}

// Style for the overall console container
fn console_style() -> container::Style {
    iced::widget::container::Style {
        background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
        border: Border {
            color: Color::from_rgb(0.3, 0.3, 0.3),
            width: 2.0,
            radius: Radius::new(0),
        },
        text_color: Some(Color::from_rgb(0.9, 0.9, 0.9)),
        shadow: Shadow {
            color: Color::BLACK,
            offset: Vector::new(0.0, 4.0),
            blur_radius: 6.0,
        },
    }
}

// Style for the scrollable area
fn scrollbar_style() -> scrollable::Style {
    scrollable::Style {
        container: container::Style {
            text_color: None,
            background: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
            border: Border {
                color: Color::TRANSPARENT,
                width: 0.0,
                radius: Radius::from(0.0),
            },
            shadow: Shadow {
                color: Color::TRANSPARENT,
                offset: Vector::new(0.0, 0.0),
                blur_radius: 0.0,
            },
        },
        vertical_rail: scrollable::Rail {
            background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
            border: Border {
                color: Color::from_rgb(0.3, 0.3, 0.3),
                width: 1.0,
                radius: Radius::from(3.0),
            },
            scroller: scrollable::Scroller {
                color: Color::from_rgb(0.6, 0.6, 0.6),
                border: Border {
                    color: Color::from_rgb(0.4, 0.4, 0.4),
                    width: 1.0,
                    radius: Radius::from(3.0),
                },
            },
        },
        horizontal_rail: scrollable::Rail {
            background: Some(Background::Color(Color::from_rgb(0.2, 0.2, 0.2))),
            border: Border {
                color: Color::from_rgb(0.3, 0.3, 0.3),
                width: 1.0,
                radius: Radius::from(3.0),
            },
            scroller: scrollable::Scroller {
                color: Color::from_rgb(0.6, 0.6, 0.6),
                border: Border {
                    color: Color::from_rgb(0.4, 0.4, 0.4),
                    width: 1.0,
                    radius: Radius::from(3.0),
                },
            },
        },
        gap: Some(Background::Color(Color::from_rgb(0.1, 0.1, 0.1))),
    }
}

fn button_style(is_selected: bool) -> button::Style {
    if is_selected {
        button::Style {
            background: Some(Background::Color(Color::from_rgb(0.3, 0.7, 0.3))), // Light green for selected
            border: Border {
                color: Color::from_rgb(0.2, 0.5, 0.2), // Darker green border
                width: 2.0,                            // Slightly thicker border for emphasis
                radius: Radius::from(8.0),             // Rounded corners
            },
            text_color: Color::WHITE, // White text for readability
            shadow: Shadow {
                color: Color::BLACK,           // Dark shadow
                offset: Vector::new(0.0, 2.0), // Slight shadow offset
                blur_radius: 4.0,              // Subtle blur
            },
        }
    } else {
        button::Style {
            background: Some(Background::Color(Color::from_rgb(0.3, 0.3, 0.3))), // Dark gray for unselected
            border: Border {
                color: Color::from_rgb(0.2, 0.2, 0.2), // Slightly lighter gray border
                width: 1.0,                            // Standard border width
                radius: Radius::from(5.0),             // Slightly less rounded corners
            },
            text_color: Color::WHITE, // White text for contrast
            shadow: Shadow {
                color: Color::BLACK,           // Minimal shadow
                offset: Vector::new(0.0, 1.0), // Small shadow offset
                blur_radius: 2.0,              // Subtle blur for unselected state
            },
        }
    }
}
