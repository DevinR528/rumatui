use rumatui_tui::backend::TestBackend;
use rumatui_tui::buffer::Buffer;
use rumatui_tui::layout::Rect;
use rumatui_tui::style::{Color, Style};
use rumatui_tui::widgets::{Block, Borders};
use rumatui_tui::Terminal;

#[test]
fn it_draws_a_block() {
    let backend = TestBackend::new(10, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|mut f| {
            let block = Block::default()
                .title("Title")
                .borders(Borders::ALL)
                .title_style(Style::default().fg(Color::LightBlue));
            f.render_widget(
                block,
                Rect {
                    x: 0,
                    y: 0,
                    width: 8,
                    height: 8,
                },
            );
        })
        .unwrap();
    let mut expected = Buffer::with_lines(vec![
        "┌Title─┐  ",
        "│      │  ",
        "│      │  ",
        "│      │  ",
        "│      │  ",
        "│      │  ",
        "│      │  ",
        "└──────┘  ",
        "          ",
        "          ",
    ]);
    for x in 1..=5 {
        expected.get_mut(x, 0).set_fg(Color::LightBlue);
    }
    assert_eq!(&expected, terminal.backend().buffer());
}
