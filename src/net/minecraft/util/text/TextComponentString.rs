use crate::net::minecraft::util::text::ITextComponent::ITextComponent;

pub fn TextComponentString(text: impl Into<String>) -> ITextComponent {
    ITextComponent::fromPlainText(text)
}
