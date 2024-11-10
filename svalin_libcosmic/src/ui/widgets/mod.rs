pub mod form;

pub fn form<'a, Message>() -> form::Form<'a, Message> {
    form::Form::new()
}
