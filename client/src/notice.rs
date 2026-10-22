//! Why we're back at the menu: one notice (title, body, maybe Retry), shown over the menu
//! until closed. Every way out of a match that the player should hear about ends here.
//! Enter retries (or closes), Esc closes. Drawn by `status_screens`.
use crate::menu::AppState;
use crate::ui::{UiAction, UiActions};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    Error,
    Info,
}

#[derive(Resource, Clone, Debug)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub kind: NoticeKind,
    /// Retry joins the same match again (`GameConfig` is kept).
    pub retry: bool,
}

impl Notice {
    pub fn error(title: &str, body: impl Into<String>) -> Notice {
        Notice { title: title.into(), body: body.into(), kind: NoticeKind::Error, retry: false }
    }

    pub fn info(title: &str, body: impl Into<String>) -> Notice {
        Notice { title: title.into(), body: body.into(), kind: NoticeKind::Info, retry: false }
    }

    pub fn with_retry(mut self) -> Notice {
        self.retry = true;
        self
    }
}

/// Enter and Esc (the menu runs these before its own actions).
pub(crate) fn keys(keys: Res<ButtonInput<KeyCode>>, notice: Res<Notice>, mut actions: ResMut<UiActions>) {
    if keys.just_pressed(KeyCode::Enter) {
        actions.0.push(if notice.retry { UiAction::NoticeRetry } else { UiAction::NoticeClose });
    } else if keys.just_pressed(KeyCode::Escape) {
        actions.0.push(UiAction::NoticeClose);
    }
}

/// Take the notice's own actions out of the queue (the menu handles the rest).
pub(crate) fn actions(mut commands: Commands, mut actions: ResMut<UiActions>, notice: Res<Notice>, mut next: ResMut<NextState<AppState>>) {
    let mut close = false;
    actions.0.retain(|a| match a {
        UiAction::NoticeRetry if notice.retry => {
            next.set(AppState::Connecting);
            close = true;
            false
        }
        UiAction::NoticeRetry | UiAction::NoticeClose => {
            close = true;
            false
        }
        _ => true,
    });
    if close {
        commands.remove_resource::<Notice>();
    }
}
