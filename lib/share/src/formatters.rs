use std::fmt::{Display, Formatter};
use std::path::Path;

pub struct ListFormatter<'l, I> {
    items: &'l [I],
    empty: &'static str,
    separator: &'static str,
    two_separator: Option<&'static str>,
    final_separator: Option<&'static str>,
    item_formatter: fn(&I, &mut Formatter<'_>) -> std::fmt::Result,
}

impl<'l, I> ListFormatter<'l, I> {
    pub fn new(
        items: &'l [I],
        empty: &'static str,
        separator: &'static str,
        two_separator: Option<&'static str>,
        final_separator: Option<&'static str>,
        item_formatter: fn(&I, &mut Formatter<'_>) -> std::fmt::Result,
    ) -> Self {
        Self { items, empty, separator, two_separator, final_separator, item_formatter }
    }

    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.items.len() {
            0 => {
                return write!(f, "{}", self.empty);
            },
            1 => {
                (self.item_formatter)(&self.items[0], f)?;
            },
            2 => {
                (self.item_formatter)(&self.items[0], f)?;
                write!(f, "{}", self.two_separator.unwrap_or(self.separator))?;
                (self.item_formatter)(&self.items[1], f)?;
            },
            3.. => {
                for (i, item) in self.items.iter().enumerate() {
                    if i > 0 && i < self.items.len() - 1 {
                        write!(f, "{}", self.separator)?;
                    } else if i > 0 {
                        write!(f, "{}", self.final_separator.unwrap_or(self.separator))?;
                    }
                    (self.item_formatter)(item, f)?;
                }
            },
        }
        Ok(())
    }
}


pub struct DisplayListFormatter<'l, I> {
    inner: ListFormatter<'l, I>,
}

impl<'l, I: Display> DisplayListFormatter<'l, I> {
    pub fn language_and(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" and "), Some(", and "), |item, f| write!(f, "{item}")) }
    }

    pub fn language_or(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" or "), Some(", or "), |item, f| write!(f, "{item}")) }
    }
}

impl<I: Display> Display for DisplayListFormatter<'_, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.inner.fmt(f) }
}


pub struct DebugListFormatter<'l, I> {
    inner: ListFormatter<'l, I>,
}

impl<'l, I: std::fmt::Debug> DebugListFormatter<'l, I> {
    pub fn language_and(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" and "), Some(", and "), |item, f| write!(f, "{item:?}")) }
    }

    pub fn language_or(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" or "), Some(", or "), |item, f| write!(f, "{item:?}")) }
    }
}

impl<I: std::fmt::Debug> std::fmt::Debug for DebugListFormatter<'_, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.inner.fmt(f) }
}

pub struct PathListFormatter<'l, I> {
    inner: ListFormatter<'l, I>,
}

impl<'l, I: AsRef<Path>> PathListFormatter<'l, I> {
    pub fn language_and(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" and "), Some(", and "), |item, f| write!(f, "{}", item.as_ref().display())) }
    }

    pub fn language_or(items: &'l [I]) -> Self {
        Self { inner: ListFormatter::new(items, "<none>", ", ", Some(" or "), Some(", or "), |item, f| write!(f, "{}", item.as_ref().display())) }
    }
}

impl<I: AsRef<Path>> std::fmt::Display for PathListFormatter<'_, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result { self.inner.fmt(f) }
}





static BLOCK_SEPARATOR: &str = "--------------------------------------------------------------------------------";
pub struct BlockFormatter<I: Display, T: Display = &'static str> {
    title: Option<T>,
    item:  I,

    separator: Option<&'static str>,
}

impl<I: Display, T: Display> BlockFormatter<I, T> {
    pub fn new(title: T, item: I) -> Self { Self { title: Some(title), item, separator: None } }
}

impl<I: Display> BlockFormatter<I> {
    pub fn no_title(item: I) -> Self { Self { title: None, item, separator: None } }
}

impl<T: Display, I: Display> Display for BlockFormatter<T, I> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let separator = self.separator.unwrap_or(BLOCK_SEPARATOR);
        if let Some(ref title) = self.title {
            writeln!(f, "{separator}")?;
            writeln!(f, "{title}")?;
        }
        writeln!(f, "{separator}")?;
        writeln!(f, "{}", self.item)?;
        writeln!(f, "{separator}")?;

        Ok(())
    }
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn test_display_list_formatter() {
        let mut x = vec![];
        assert_eq!(format!("{}", DisplayListFormatter::language_and(&x)), String::from("<none>"));

        x.push("a");
        assert_eq!(format!("{}", DisplayListFormatter::language_and(&x)), String::from("a"));
        assert_eq!(format!("{}", DisplayListFormatter::language_or(&x)), String::from("a"));

        x.push("b");
        assert_eq!(format!("{}", DisplayListFormatter::language_and(&x)), String::from("a and b"));
        assert_eq!(format!("{}", DisplayListFormatter::language_or(&x)), String::from("a or b"));

        x.push("c");
        assert_eq!(format!("{}", DisplayListFormatter::language_and(&x)), String::from("a, b, and c"));
        assert_eq!(format!("{}", DisplayListFormatter::language_or(&x)), String::from("a, b, or c"));
    }

    #[test]
    fn test_debug_list_formatter() {
        let mut x = vec![];
        assert_eq!(format!("{:?}", DebugListFormatter::language_and(&x)), String::from("<none>"));

        x.push("a");
        assert_eq!(format!("{:?}", DebugListFormatter::language_and(&x)), String::from(r#""a""#));
        assert_eq!(format!("{:?}", DebugListFormatter::language_or(&x)), String::from(r#""a""#));

        x.push("b");
        assert_eq!(format!("{:?}", DebugListFormatter::language_and(&x)), String::from(r#""a" and "b""#));
        assert_eq!(format!("{:?}", DebugListFormatter::language_or(&x)), String::from(r#""a" or "b""#));

        x.push("c");
        assert_eq!(format!("{:?}", DebugListFormatter::language_and(&x)), String::from(r#""a", "b", and "c""#));
        assert_eq!(format!("{:?}", DebugListFormatter::language_or(&x)), String::from(r#""a", "b", or "c""#));
    }

    #[test]
    fn test_path_list_formatter() {
        let mut x = vec![];
        assert_eq!(format!("{}", PathListFormatter::language_and(&x)), String::from("<none>"));

        x.push(PathBuf::from("a"));
        assert_eq!(format!("{}", PathListFormatter::language_and(&x)), String::from("a"));
        assert_eq!(format!("{}", PathListFormatter::language_or(&x)), String::from("a"));

        x.push(PathBuf::from("b"));
        assert_eq!(format!("{}", PathListFormatter::language_and(&x)), String::from("a and b"));
        assert_eq!(format!("{}", PathListFormatter::language_or(&x)), String::from("a or b"));

        x.push(PathBuf::from("c"));
        assert_eq!(format!("{}", PathListFormatter::language_and(&x)), String::from("a, b, and c"));
        assert_eq!(format!("{}", PathListFormatter::language_or(&x)), String::from("a, b, or c"));
    }

    #[test]
    fn test_block_formatter() {
        assert_eq!(
            format!("{}", BlockFormatter::new("Title", "item1\nitem2"),),
            String::from(indoc::indoc! { "
                --------------------------------------------------------------------------------
                Title
                --------------------------------------------------------------------------------
                item1
                item2
                --------------------------------------------------------------------------------
            " })
        );
        assert_eq!(
            format!("{}", BlockFormatter::no_title("item1\nitem2"),),
            String::from(indoc::indoc! { "
                --------------------------------------------------------------------------------
                item1
                item2
                --------------------------------------------------------------------------------
            " })
        );
    }
}
