mod component;
mod components;
mod database;
mod home;

use crate::component::Component;
use crate::components::{Components, CurrentComponent};
use crate::home::Home;
use iced::{
    widget::{button, column, container, row, text, Column, Text},
    Element, Task,
};

#[derive(Debug, Clone)]
pub enum Message {
    InitializeComponents(Components),
    InitializeHomeComponent,
    HomeComponentInitialized(Home),
}

async fn initialize_component<T: Component>(mut component: T) -> T {
    component.initialize_component().await;
    component
}

struct Crm {
    current_component: CurrentComponent,
    components: Option<Components>,
}

impl Crm {
    fn setup() -> (Self, Task<Message>) {
        (
            Self {
                current_component: CurrentComponent::Home,
                components: None,
            },
            Task::perform(Components::new(), |components| {
                Message::InitializeComponents(components)
            }),
        )
    }
    fn title(&self) -> String {
        String::from("CRM")
    }
    fn view(&self) -> Element<Message> {
        if self.components.is_none() {
            column![container("loading")].into()
        } else {
            let components = self.components.clone().unwrap();
            match self.current_component {
                CurrentComponent::Home => {
                    let home_component = components.home;
                    if !home_component.tables.is_none() {
                        Column::with_children(
                            home_component
                                .tables
                                .unwrap_or_default()
                                .into_iter()
                                .map(|table| Text::new(table.table_name).into())
                                .collect::<Vec<_>>(),
                        )
                        .into()
                    } else {
                        column!(text("loading")).into()
                    }
                }
            }
        }
    }
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::InitializeComponents(components) => {
                self.components = Some(components);
                Task::done(Message::InitializeHomeComponent)
            }
            Message::InitializeHomeComponent => {
                let home_component = self.components.clone().unwrap().home;
                Task::perform(
                    async move { initialize_component::<Home>(home_component).await },
                    |home| Message::HomeComponentInitialized(home),
                )
            }
            Message::HomeComponentInitialized(home) => {
                if let Some(components) = &mut self.components {
                    components.home = home;
                }
                Task::none()
            }
        }
    }
}

fn main() -> iced::Result {
    iced::application(Crm::title, Crm::update, Crm::view).run_with(Crm::setup)
}
