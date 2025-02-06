use crate::config;
use crate::resources;
use crate::sim_monitor;
use crate::tray;

use iced::widget::checkbox;
use iced::widget::container;
use iced::widget::{button, column, row, text, text_input, Column, Container, Space};
use iced::window;
use iced::Length::{self, Fill};
use iced::{keyboard, Element, Padding};
use iced::{Subscription, Task};
use iced_aw::widgets::number_input;

#[derive(Debug, Clone)]
pub enum Message {
    WindowOpened(window::Id),
    WindowClosed(window::Id),
    Quit,

    MqttHostChanged(String),
    MqttPortChanged(u16),
    MqttUserChanged(String),
    MqttPasswordChanged(String),
    ApplyMqttConfig,

    SimUpdated(sim_monitor::Event),
    TrayEvent(tray::TrayEventType),

    ConfigFileEvent(config::Event),

    SettingsPressed,
    HomePressed,
    MqttToggled(bool),
}

enum State {
    WaitingForBackendConnection,
    ConnectedToBackend(sim_monitor::Connection),
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::ConnectedToBackend(_connection) => write!(f, "Ready"),
            State::WaitingForBackendConnection => write!(f, "Waiting for backend connection"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Home,
    Settings,
}

pub struct IracingMonitorGui {
    config: config::AppConfig,

    // mqtt_host: String,
    // mqtt_port: String,
    // mqtt_user: String,
    // mqtt_password: String,
    // port_is_valid: bool,
    state: State,
    connected_to_sim: bool,
    sim_state: Option<sim_monitor::SimMonitorState>,

    // tray_icon: Option<TrayIcon>,
    tray: Option<tray::Connection>,

    window_id: Option<window::Id>,

    screen: Screen,
}

impl IracingMonitorGui {
    pub fn new() -> (Self, Task<Message>) {
        let settings = Self::window_settings();
        let config = config::get_app_config();
        let (id, open) = window::open(settings);

        (
            Self {
                config: config.clone(),

                // mqtt_host: config.mqtt.host,
                // mqtt_port: config.mqtt.port.to_string(),
                // mqtt_user: config.mqtt.user,
                // mqtt_password: config.mqtt.password,
                // port_is_valid: true,
                state: State::WaitingForBackendConnection,
                connected_to_sim: false,
                sim_state: None,

                // tray_icon: Some(new_tray_icon()), // panic, gtk has hot been initialized, call gtk::init first
                tray: None,

                window_id: Some(id),
                screen: Screen::Home,
            },
            Task::batch([
                open.map(Message::WindowOpened),
            ])
        )
    }

    fn window_settings() -> iced::window::Settings {
        iced::window::Settings {
            size: iced::Size {
                width: 400.0 * 1.618,
                height: 400.0,
            },
            min_size: Some(iced::Size {
                width: 400.0 * 1.618,
                height: 400.0,
            }),
            icon: load_icon(),
            ..Default::default()
        }
    }

    pub fn theme(&self, _window_id: iced::window::Id) -> iced::Theme {
        // iced::Theme::Oxocarbon
        iced::Theme::Dark
    }

    pub fn title(&self, _window_id: iced::window::Id) -> String {
        // "IRacingMonitor - Iced".to_string()
        // format!("IRacingMonitor {:?}", window_id)
        "iRacingMonitor".to_string()
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::MqttHostChanged(value) => {
                self.config.mqtt.host = value;
            }
            Message::MqttPortChanged(value) => {
                self.config.mqtt.port = value;
            }
            Message::MqttUserChanged(value) => {
                self.config.mqtt.user = value;
            }
            Message::MqttPasswordChanged(value) => {
                self.config.mqtt.password = value;
            }
            Message::ApplyMqttConfig => {
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to save config to file: {err}");
                }

                let msg = sim_monitor::Message::UpdateConfig(self.config.mqtt.clone());
                match &mut self.state {
                    State::ConnectedToBackend(connection) => {
                        connection.send(msg);
                    }
                    State::WaitingForBackendConnection => {
                        log::warn!("Invalid state, waiting for backend")
                    }
                }
            }
            Message::WindowOpened(id) => {
                if id != self.window_id.unwrap() {
                    log::warn!("Window ID mismatch");
                }
            }
            Message::WindowClosed(_id) => {
                log::info!("Window closed");
                self.window_id = None;
            }
            Message::SimUpdated(event) => {
                log::debug!("SimUpdated message received! ({event})");
                // self.iracing_connection_status = format!("{event}");

                match event {
                    sim_monitor::Event::Ready(connection) => {
                        self.state = State::ConnectedToBackend(connection);
                        log::info!("Backend ready, waiting for game");
                    }
                    sim_monitor::Event::DisconnectedFromSim(state) => {
                        if self.connected_to_sim {
                            log::info!("Disconnected from game");
                            self.connected_to_sim = false;
                        }
                        self.sim_state = Some(state);
                    }
                    sim_monitor::Event::ConnectedToSim(state) => {
                        if !self.connected_to_sim {
                            log::info!("Connected to game");
                            self.connected_to_sim = true;
                        }
                        self.sim_state = Some(state);
                    }
                }
            }
            Message::TrayEvent(event) => {
                log::info!("Tray event: {event:?}");
                match event {
                    tray::TrayEventType::MenuItemClicked(id) => {
                        // if id.0 == "quit" {
                        //     Task::done(Message::Quit)
                        // } else {
                        //     Task::none()
                        // }
                        match id.0.as_str() {
                            // TODO: matching on strings is bad and you should feel bad
                            "quit" => return Task::done(Message::Quit),
                            "options" => return self.open_window(),
                            _ => {
                                log::warn!("Unknown tray menu item clicked: {}", id.0);
                                return Task::none();
                            }
                        }
                    }
                    tray::TrayEventType::Connected(connection) => {
                        self.tray = Some(connection);
                    } // tray::TrayEventType::IconClicked => {
                      //     self.open_window()
                      // }
                }
            }
            Message::ConfigFileEvent(event) => match event {
                config::Event::Modified(config) | config::Event::Created(config) => {
                    log::info!("Config file updated: {config:?}");
                }
                config::Event::Deleted(path) => {
                    log::info!("Config file {path:?} deleted");
                }
            },
            Message::SettingsPressed => {
                self.screen = Screen::Settings;
            }
            Message::HomePressed => {
                self.screen = Screen::Home;
            }
            Message::MqttToggled(state) => {
                self.config.mqtt_enabled = state;
            }
            Message::Quit => {
                log::info!("Quitting application!");

                // save config
                log::info!("Saving config to file");
                if let Err(err) = self.config.save() {
                    log::warn!("Failed to save config to file: {err}");
                }

                // kill tray
                if let Some(tray) = &mut self.tray {
                    tray.send(tray::Message::Quit);
                }

                return iced::exit();
            }
        }
        Task::none()
    }

    fn open_window(&mut self) -> Task<Message> {
        if self.window_id.is_none() {
            log::debug!("Opening settings window");
            let settings = Self::window_settings();
            let (id, open) = window::open(settings);
            self.window_id = Some(id);
            open.map(Message::WindowOpened)
        } else {
            log::info!("Settings window already open");
            Task::none()
        }
    }

    fn home(&self) -> Column<Message> {
        column![
            // text("iRacing Home Assistant Monitor").size(28),
            // Space::new(Length::Shrink, Length::Fixed(16.)),
            row![
                // text(),
                checkbox("Publish to MQTT", self.config.mqtt_enabled)
                    .on_toggle(Message::MqttToggled),
            ],
            // Space::new(Length::Shrink, Length::Fixed(16.)),
            // button("Settings").on_press(Message::SettingsPressed),
        ]
    }

    fn settings(&self) -> Column<Message> {
        let text_width = 100;
        let row_spacing = 4.0;
        column![
            // button("Back").on_press(Message::HomePressed),
            // Space::new(Length::Shrink, Length::Fixed(16.)),
            text("MQTT settings"),
            Space::new(Length::Shrink, Length::Fixed(16.)),
            row![
                text("Host").width(text_width),
                text_input("Host", &self.config.mqtt.host).on_input(Message::MqttHostChanged),
            ]
            .align_y(iced::alignment::Vertical::Center),
            Space::new(Length::Shrink, Length::Fixed(row_spacing)),
            row![
                text("Port").width(text_width),
                number_input(self.config.mqtt.port, 0..65535, Message::MqttPortChanged)
                    .ignore_buttons(true)
                    .width(Fill),
            ]
            .align_y(iced::alignment::Vertical::Center),
            Space::new(Length::Shrink, Length::Fixed(row_spacing)),
            row![
                text("User").width(text_width),
                text_input("User", &self.config.mqtt.user).on_input(Message::MqttUserChanged),
            ]
            .align_y(iced::alignment::Vertical::Center),
            Space::new(Length::Shrink, Length::Fixed(row_spacing)),
            row![
                text("Password").width(text_width),
                text_input("Password", &self.config.mqtt.password)
                    .on_input(Message::MqttPasswordChanged)
                    .secure(true),
            ]
            .align_y(iced::alignment::Vertical::Center),
            Space::new(Length::Shrink, Length::Fixed(16.)),
            button("Apply MQTT config").on_press(Message::ApplyMqttConfig),
        ]
    }

    pub fn view(&self, _window_id: iced::window::Id) -> Element<Message> {
        // main screen
        let screen = match self.screen {
            Screen::Home => self.home(),
            Screen::Settings => self.settings(),
        };

        // bottom status messages
        let last_message = if let Some(sim_state) = &self.sim_state {
            sim_state.timestamp.clone()
        } else {
            "None".to_string()
        };

        let status = column![
            text(self.state.to_string()).size(16),
            text(format!(
                "Session type: {}",
                if let Some(sim_state) = &self.sim_state {
                    sim_state.current_session_type.clone()
                } else {
                    "None".to_string()
                }
            )),
            text(format!("Last message: {last_message}")),
        ];

        // this seems like a lot of boilerplate to style a container, but it works
        pub fn left_container_style(
            _theme: &iced::widget::Theme,
        ) -> iced::widget::container::Style {
            iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::TRANSPARENT)),
                border: iced::border::rounded(2),
                ..iced::widget::container::Style::default()
            }
        }
        pub fn container_style(_theme: &iced::widget::Theme) -> iced::widget::container::Style {
            // see here fore inspiration: https://docs.rs/iced_widget/0.13.1/src/iced_widget/container.rs.html#682
            iced::widget::container::Style {
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    1., 1., 1., 0.01,
                ))),
                border: iced::border::rounded(2),
                ..iced::widget::container::Style::default()
            }
        }

        pub fn button_style(
            theme: &iced::widget::Theme,
            status: iced::widget::button::Status,
            screen: &Screen,
            my_screen: &Screen,
        ) -> iced::widget::button::Style {
            use iced::border;
            use iced::theme::palette;
            use iced::widget::button::{Status, Style};
            use iced::Background;

            // from https://docs.iced.rs/src/iced_widget/button.rs.html#591
            fn styled(pair: palette::Pair) -> Style {
                Style {
                    background: Some(Background::Color(pair.color)),
                    text_color: pair.text,
                    border: border::rounded(2),
                    ..Style::default()
                }
            }

            fn disabled(style: Style) -> Style {
                Style {
                    background: style
                        .background
                        .map(|background| background.scale_alpha(0.5)),
                    text_color: style.text_color.scale_alpha(0.5),
                    ..style
                }
            }

            let palette = theme.extended_palette();
            let base = if screen == my_screen {
                styled(palette.primary.strong)
            } else {
                let pair = palette::Pair {
                    text: palette.primary.strong.text,
                    color: iced::Color::from_rgb(0.1, 0.1, 0.1), // background color
                };
                styled(pair)
            };

            match status {
                Status::Active | Status::Pressed => base,
                Status::Hovered => Style {
                    background: Some(Background::Color(palette.primary.base.color)),
                    ..base
                },
                Status::Disabled => disabled(base),
            }
        }

        // let style = container::background(iced::Color::from_rgb(0.1, 0.1, 0.1));
        let left_menu = column![container(
            column![
                button(
                    // home_text
                    "Home"
                )
                .width(Length::Fill)
                // .style(button_style)
                .style(|theme, status| button_style(theme, status, &self.screen, &Screen::Home))
                .on_press(Message::HomePressed),
                button(
                    // settings_text
                    "Settings"
                )
                .width(Length::Fill)
                .style(|theme, status| button_style(theme, status, &self.screen, &Screen::Settings))
                .on_press(Message::SettingsPressed),
            ] // .width(Length::Fixed(96.)),
        )
        // .width(Length::Fixed(96.))
        // .padding(10)
        // .center(800)
        // .center(Fill)
        .align_left(Length::Fixed(150.))
        .align_top(Length::Fill)
        // .style(container::rounded_box)
        .style(left_container_style)]
        .padding(Padding::new(0.).right(10));

        // container(
        column![
            text("iRacing Home Assistant Monitor").size(28),
            row![
                left_menu,
                column![
                    // main screen
                    container(screen,)
                        .align_left(Length::Fill)
                        .align_top(Length::Fill)
                        .style(container_style)
                        .padding(6),
                    // status messages
                    // Space::new(Length::Shrink, Length::Fill), // push status to bottom
                    Space::new(Length::Shrink, Length::Fixed(6.)),
                    container(
                        row![
                            status,
                            Space::new(Length::Fill, Length::Shrink),
                            button("Quit")
                                // .width(Length::Fixed(64.))
                                .clip(false)
                                .on_press(Message::Quit),
                        ]
                        .align_y(iced::alignment::Vertical::Bottom) // align to bottom
                    )
                    .align_left(Length::Fill)
                    .align_bottom(Length::Shrink)
                    .style(container_style)
                    .padding(6),
                ]
            ]
        ]
        .padding(10) // pad the whole container (distance to window edges)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        fn handle_hotkey(key: keyboard::Key, modifiers: keyboard::Modifiers) -> Option<Message> {
            match (key.as_ref(), modifiers) {
                // quit on Ctrl+Q
                (keyboard::Key::Character("q"), modifiers) if modifiers.control() => {
                    Some(Message::Quit)
                }
                _ => None,
            }
        }

        Subscription::batch(vec![
            window::close_events().map(Message::WindowClosed),
            keyboard::on_key_press(handle_hotkey),
            Subscription::run(sim_monitor::connect).map(Message::SimUpdated),
            Subscription::run(tray::tray_subscription).map(Message::TrayEvent),
            Subscription::run(config::watch).map(Message::ConfigFileEvent),
        ])
    }
}

fn load_icon() -> Option<iced::window::Icon> {
    match iced::window::icon::from_file_data(resources::ICON_BYTES, None) {
        Ok(icon) => Some(icon),
        Err(e) => {
            log::warn!("Failed to load icon: {e}");
            None
        }
    }
}
