use std::marker::PhantomData;

use super::arena_id::*;

#[derive(Clone, Copy)]
pub struct List<T: Copy> {
    first: Id<Node<T>>,
    last: Id<Node<T>>,
}

#[derive(Copy, Clone)]
struct Node<T: Copy> {
    value: T,
    next: Id<Node<T>>,
}

impl<T: Copy> List<T> {
    pub fn new() -> Self {
        Self {
            first: Id::invalid(),
            last: Id::invalid(),
        }
    }

    pub fn add(&mut self, arena: &mut Arena, val: T) {
        let node = Node {
            value: val,
            next: Id::invalid(),
        };
        let node = arena.alloc::<Node<T>>(node);

        if self.first.is_invalid() {
            self.first = node;
            self.last = node;
        } else {
            let last = arena.get_mut(self.last);
            last.next = node;
            self.last = node;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.first.is_invalid()
    }

    pub fn first(&self, arena: &Arena) -> Option<T> {
        if !self.first.is_invalid() {
            let first = arena.get(self.first);
            return Some(first.value);
        }
        None
    }

    pub fn iter<'a>(&self, arena: &'a Arena) -> ListIter<'a, T> {
        ListIter {
            curr: self.first,
            phantom: PhantomData,
            arena,
        }
    }
}

pub struct ListIter<'a, T: Copy> {
    curr: Id<Node<T>>,
    phantom: PhantomData<&'a T>,
    arena: &'a Arena,
}

impl<'a, T: Copy> Iterator for ListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_invalid() {
            None
        } else {
            let curr = self.arena.get(self.curr);
            self.curr = curr.next;
            Some(&curr.value)
        }
    }
}
