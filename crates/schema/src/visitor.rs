use std::collections::HashMap;

use crate::{Annotations, DataType, Entity, Event, Field, Identifier, Record, Schema};

/// Reference to a schema element.
#[derive(Clone, Copy)]
pub enum Element<'s> {
    Schema(&'s Schema),
    Annotations(&'s Annotations),
    Entity(&'s Entity),
    Event(&'s Event),
    Field(&'s Field),
    Record(&'s Record),
    DataType(&'s DataType),
}

/// Iterates through a schema.
pub struct Cursor<'s>(Vec<Element<'s>>);

impl<'s> Cursor<'s> {
    pub fn new(schema: &'s Schema) -> Self {
        Self(vec![Element::Schema(schema)])
    }
    pub fn current(&self) -> Element<'s> {
        todo!()
    }
    pub fn previous(&self) -> Option<Element<'s>> {
        todo!()
    }
    pub fn root(&self) -> &'s Schema {
        todo!()
    }
    pub fn elements(&self) -> &[Element<'s>] {
        todo!()
    }
}

pub trait Visitor {
    type Output;
    fn visit(&mut self, cursor: &Cursor, index: &SchemaIndex);
    fn finish(self) -> Self::Output;
}

/// Index computed once per walk that visitors can leverage to quickly look up
/// internal references.
pub struct SchemaIndex<'s> {
    records: HashMap<Identifier, &'s Record>,
    entities: HashMap<Identifier, &'s Entity>,
}

/// Walk a schema with the supplied visitors
pub fn walk<T>(schema: &Schema, mut visitor: T) -> <T as Visitor>::Output
where
    T: Visitor,
{
    // build schema index
    // traverse schema by iterating with the cursor
    todo!()
}

// Macro to create impls for tuples of visitors, so output can be collected
// without having to upcast.
macro_rules! tuple_impls {
    ($($T:ident => $idx:tt),+) => {
        impl<$($T: Visitor),+> Visitor for ($($T,)+) {
            type Output = ($($T::Output,)+);
            fn visit(&mut self, cursor: &Cursor, index: &SchemaIndex) {
                $( self.$idx.visit(cursor, index); )+
            }
            fn finish(self) -> Self::Output {
                ($( self.$idx.finish(), )+)
            }
        }
    };
}
tuple_impls!(A => 0, B => 1);
tuple_impls!(A => 0, B => 1, C => 2);
tuple_impls!(A => 0, B => 1, C => 2, D => 3);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10);
tuple_impls!(A => 0, B => 1, C => 2, D => 3, E => 4, F => 5, G => 6, H => 7, I => 8, J => 9, K => 10, L => 11);

#[cfg(test)]
mod test {
    use super::*;

    // Stateless visitor
    struct FooVisitor;

    impl Visitor for FooVisitor {
        type Output = ();
        fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {}
        fn finish(self) -> Self::Output {}
    }

    // Stateful visitor
    struct BarVisitor {
        beers: u8,
    }

    impl Visitor for BarVisitor {
        type Output = u8;
        fn visit(&mut self, _cursor: &Cursor, _index: &SchemaIndex) {
            self.beers += 1
        }
        fn finish(self) -> u8 {
            self.beers
        }
    }

    #[cfg(test)]
    fn test() {
        let schema = Schema {
            name: Identifier::try_new("Foo").unwrap(),
            entities: todo!(),
            records: todo!(),
            annotations: todo!(),
        };

        let (foo, bar) = walk(&schema, (FooVisitor, BarVisitor { beers: 0 }));
    }
}
