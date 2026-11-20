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
    Provider(usize),
}

impl SidebarItem {
    pub fn all(provider_count: usize) -> impl Iterator<Item = SidebarItem> {
        [
            SidebarItem::Downloads,
            SidebarItem::History,
            SidebarItem::Queue,
            SidebarItem::Settings,
        ]
        .into_iter()
        .chain((0..provider_count).map(SidebarItem::Provider))
    }

    pub fn step(self, forward: bool, provider_count: usize) -> SidebarItem {
        let items: Vec<_> = Self::all(provider_count).collect();
        let index = items.iter().position(|item| *item == self).unwrap_or(0);
        let next = if forward {
            (index + 1) % items.len()
        } else {
            (index + items.len() - 1) % items.len()
        };
        items[next]
    }
}

pub struct Sidebar<'a> {
    selected: SidebarItem,
    focused: bool,
    providers: &'a [&'a str],
}

impl<'a> Sidebar<'a> {
    pub fn new(selected: SidebarItem, focused: bool, providers: &'a [&'a str]) -> Self {
        Self {
            selected,
            focused,
            providers,
        }
    }
}

impl Widget for Sidebar<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default()
        };
        let block = Block::bordered().title(" FLUX ").border_style(border_style);
        let inner = block.inner(area);
        block.render(area, buf);

        let mut lines = vec![
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
        ];
        for (index, name) in self.providers.iter().enumerate() {
            lines.push(item_line(name, SidebarItem::Provider(index), self.selected));
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn step_wraps_in_both_directions() {
        let last = SidebarItem::Provider(1);
        assert!(last.step(true, 2) == SidebarItem::Downloads);
        assert!(SidebarItem::Downloads.step(false, 2) == last);
        assert!(SidebarItem::Settings.step(true, 2) == SidebarItem::Provider(0));
        assert!(SidebarItem::Settings.step(true, 0) == SidebarItem::Downloads);
    }
}
