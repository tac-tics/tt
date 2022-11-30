use crate::TermTextState;
use tt::message::ServerMessage;
use log::*;


pub fn render(state: &TermTextState) -> Vec<ServerMessage> {
    let mut messages = Vec::new();

    let size = state.size;

    info!("send_update()");
    let pos = (0, 0);
    let mut cursor_pos = pos;

    let mut lines = Vec::new();
    let show_line_numbers = true;

    if let Some(buffer) = state.current_buffer() {
        let data = buffer.data.clone();

        let mut line = if show_line_numbers {
            "     1 | ".to_string()
        } else {
             String::new()
        };

        for ch in data.chars() {
            if ch == '\n' {
                lines.push(line);
                line = if show_line_numbers {
                    format!("{:6} | ", cursor_pos.1 + 2)
                } else {
                     String::new()
                };
                cursor_pos.0 = 0;
                cursor_pos.1 += 1;
            } else if ch == '\t' {
                line.push_str("    ");
                cursor_pos.0 += 4;
            } else {
                line.push(ch);
                cursor_pos.0 += 1;
            }
        }

        lines.push(line);

        messages.push(ServerMessage::Update(pos, size, lines));
    }

    let status_pos = if size.1 > 0 {
        (0, size.1 - 1 as u16)
    } else {
        (0, 0)
    };
    let status_size = (size.0, 1);
    let mut status_line = "tt: ".to_string();
    status_line.push_str(&format!("{:?}", state.mode));
    if let Some(command) = &state.command {
        status_line.push_str(&format!(" {}", command));
    }

    if let Some(buffer) = state.current_buffer() {
        if let Some(path) = &buffer.path {
            status_line.push_str(&format!(" {path:?}"));
        }
    }
    messages.push(ServerMessage::Update(status_pos, status_size, vec![status_line]));

    if show_line_numbers {
        cursor_pos.0 += 9;
    }
    messages.push(ServerMessage::Cursor(cursor_pos));

    messages
}
