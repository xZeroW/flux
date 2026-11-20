use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Paragraph, Widget},
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SidebarItem {
    Downloads,
    History,
    Queue,
    Settings,
    MediaFire,
    Mega,
    GoogleDrive,
    Torrents,
}

pub struct Sidebar {
    selected: SidebarItem,
}

impl Sidebar {
    pub fn new(selected: SidebarItem) -> Self {
        Self { selected }
    }
}

impl Widget for Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::bordered().title(" FLUX ");
        let inner = block.inner(area);
        block.render(area, buf);

        let lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                "  GENERAL",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            item_line("Downloads", SidebarItem::Downloads, self.selected),
            item_line("History", SidebarItem::History, self.selected),
            item_line("Queue", SidebarItem::Queue, self.selected),
            item_line("Settings", SidebarItem::Settings, self.selected),
            Line::from(""),
            Line::from(Span::styled(
                "  PROVIDERS",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            item_line("MediaFire", SidebarItem::MediaFire, self.selected),
            item_line("MEGA", SidebarItem::Mega, self.selected),
            item_line("Google Drive", SidebarItem::GoogleDrive, self.selected),
            item_line("Torrents", SidebarItem::Torrents, self.selected),
        ];

        Paragraph::new(lines).render(inner, buf);
    }
}

fn item_line(label: &str, item: SidebarItem, selected: SidebarItem) -> Line<'static> {
    let is_selected = item == selected;
    let prefix = if is_selected { "  ▸ " } else { "    " };
    let style = if is_selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };

    Line::from(Span::styled(format!("{prefix}{label}"), style))
}
