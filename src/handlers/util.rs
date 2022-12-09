use std::collections::HashMap;

use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};

use crate::{
    auth::user::{User, UserAction},
    events::{Event, EventInner},
};

use super::users::Claim;

fn decode_token(token: &str, jwt_secret: &str) -> Option<String> {
    match decode::<Claim>(
        token,
        &DecodingKey::from_secret(jwt_secret.as_bytes()),
        &Validation::new(Algorithm::HS512),
    ) {
        Ok(t) => Some(t.claims.uid),
        Err(_) => None,
    }
}

fn decode_no_verify(token: &str) -> Option<String> {
    let mut no_verify = Validation::new(Algorithm::HS512);
    no_verify.insecure_disable_signature_validation();
    match decode::<Claim>(
        token,
        &DecodingKey::from_secret("secret".as_bytes()),
        &no_verify,
    ) {
        Ok(t) => Some(t.claims.uid),
        Err(_) => None,
    }
}

pub fn try_auth(token: &str, users: &HashMap<String, User>) -> Option<User> {
    let claimed_uid = decode_no_verify(token)?;
    let claimed_requester = users.get(&claimed_uid)?;
    let requester_uid = decode_token(token, &claimed_requester.secret)?;
    if claimed_uid != requester_uid {
        return None;
    }
    Some(claimed_requester.to_owned())
}

pub fn can_user_view_event(event: impl AsRef<Event>, user: &User) -> bool {
    match &event.as_ref().event_inner {
        EventInner::InstanceEvent(event) => {
            user.can_perform_action(&UserAction::ViewInstance(event.instance_uuid.clone()))
        }
        EventInner::UserEvent(_event) => user.can_perform_action(&UserAction::ManageUser),
        EventInner::FSEvent(_) => user.can_perform_action(&UserAction::ManageUser),
        EventInner::MacroEvent(macro_event) => {
            user.can_perform_action(&UserAction::AccessMacro(macro_event.instance_uuid.clone()))
        }
        EventInner::ProgressionEvent(progression_event) => user.can_perform_action(
            &UserAction::ViewInstance(progression_event.event_id.clone()),
        ),
    }
}

pub fn parse_bearer_token(token: &str) -> Option<String> {
    let mut split = token.split_ascii_whitespace();
    if split.next()? != "Bearer" {
        return None;
    }
    split.next().map(|s| s.to_string())
}
