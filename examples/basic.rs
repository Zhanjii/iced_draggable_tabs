//! Basic example: draggable tab bar with close buttons, custom theming, and reorder tracking.

use iced::widget::{column, container, row, text, toggler, Space};
use iced::{Element, Length, Task, Theme};
use iced_draggable_tabs::DraggableTabs;

fn main() -> iced::Result {
    iced::application(boot, update, view)
        .title(title)
        .theme(theme)
        .window_size((700.0, 450.0))
        .run()
}

struct App {
    active_tab: usize,
    tab_order: Vec<usize>,
    tabs: Vec<&'static str>,
    is_dark: bool,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(usize),
    TabsReordered(Vec<usize>),
    TabClosed(usize),
    ToggleDarkMode(bool),
}

fn title(app: &App) -> String {
    if app.active_tab < app.tabs.len() {
        format!("Draggable Tabs — {}", app.tabs[app.active_tab])
    } else {
        "Draggable Tabs Demo".to_string()
    }
}

fn theme(app: &App) -> Theme {
    if app.is_dark {
        Theme::CatppuccinMocha
    } else {
        Theme::CatppuccinLatte
    }
}

fn boot() -> (App, Task<Message>) {
    (
        App {
            active_tab: 0,
            tab_order: vec![],
            tabs: vec![
                "Dashboard",
                "Projects",
                "Settings",
                "Profile",
                "Help",
            ],
            is_dark: true,
        },
        Task::none(),
    )
}

fn update(app: &mut App, message: Message) -> Task<Message> {
    match message {
        Message::TabSelected(idx) => {
            app.active_tab = idx;
        }
        Message::TabsReordered(new_order) => {
            app.tab_order = new_order;
        }
        Message::TabClosed(idx) => {
            if app.tabs.len() > 1 {
                app.tabs.remove(idx);
                // Reset order since indices changed
                app.tab_order = (0..app.tabs.len()).collect();
                if app.active_tab >= app.tabs.len() {
                    app.active_tab = app.tabs.len() - 1;
                }
            }
        }
        Message::ToggleDarkMode(is_dark) => {
            app.is_dark = is_dark;
        }
    }
    Task::none()
}

fn view(app: &App) -> Element<'_, Message> {
    let tabs = DraggableTabs::new(
        &app.tabs,
        app.active_tab,
        Message::TabSelected,
        Message::TabsReordered,
    )
    .tab_height(40.0)
    .text_size(14.0)
    .spacing(2.0)
    .on_close(Message::TabClosed);

    let content_text = if app.active_tab < app.tabs.len() {
        match app.tabs[app.active_tab] {
            "Dashboard" => "Welcome to the Dashboard! Drag the tabs above to reorder them.",
            "Projects" => "Projects panel — view and manage your active projects.",
            "Settings" => "Settings panel — configure application preferences.",
            "Profile" => "Profile panel — update your user information.",
            "Help" => "Help panel — documentation and support resources.",
            _ => "Custom tab content.",
        }
    } else {
        "No tab selected"
    };

    let order_text = if app.tab_order.is_empty() {
        "Tab order: [default]".to_string()
    } else {
        format!("Tab order: {:?}", app.tab_order)
    };

    let dark_toggle = row![
        text("Dark Mode").size(14),
        toggler(app.is_dark).on_toggle(Message::ToggleDarkMode),
    ]
    .spacing(8);

    let info = column![
        text(content_text).size(16),
        Space::new().height(10),
        text(order_text).size(12),
        Space::new().height(6),
        text(format!("Tabs open: {}", app.tabs.len())).size(12),
    ];

    container(
        column![
            Element::from(tabs),
            Space::new().height(20),
            row![info, Space::new().width(Length::Fill), dark_toggle],
        ]
        .spacing(0)
        .padding(20),
    )
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}
