//! What the status line says for each way a login, a join or a character creation can fail.

use canastra_protocol::game::{CreationFailure, Refusal};
use canastra_protocol::login::{AuthFailure, TicketRefusal};

pub(super) fn auth(failure: AuthFailure) -> &'static str {
    match failure {
        AuthFailure::WrongCredentials => "Wrong account or password.",
        AuthFailure::Banned => "This account is banned.",
        AuthFailure::TooManyAttempts => "Too many attempts. Try again later.",
    }
}

pub(super) fn ticket(refusal: TicketRefusal) -> &'static str {
    match refusal {
        TicketRefusal::Offline => "That server is offline.",
        TicketRefusal::Full => "That server is full.",
    }
}

pub(super) fn admission(refusal: Refusal) -> &'static str {
    match refusal {
        Refusal::InvalidTicket => "The server did not accept the login ticket.",
        Refusal::Expired => "The login ticket expired. Pick the server again.",
        Refusal::Reused => "That login ticket was already used.",
        Refusal::Full => "That server is full.",
    }
}

pub(super) fn creation(failure: CreationFailure) -> &'static str {
    match failure {
        CreationFailure::InvalidName => "That name is not allowed here. Use letters and digits.",
        CreationFailure::ForbiddenName => "That name contains a forbidden word.",
        CreationFailure::NameTaken => "That name is taken.",
        CreationFailure::SlotsFull => "No free character slot on this server.",
        CreationFailure::InvalidClass => "Pick a class and a sex it allows.",
        CreationFailure::InvalidAppearance => "That appearance is not available.",
    }
}
