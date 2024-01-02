use super::*;

#[derive(Copy, Clone)]
pub struct List<T: Copy> {
    first: P<Node<T>>,
    last: P<Node<T>>,
}

#[derive(Copy, Clone)]
struct Node<T: Copy> {
    val: T,
    next: P<Node<T>>,
}

impl<T: Copy> List<T> {
    pub fn new() -> Self {
        Self {
            first: P::null(),
            last: P::null(),
        }
    }

    pub fn add(&mut self, arena: &mut Arena, val: T) {
        let mut node = arena.alloc::<Node<T>>();
        node.val = val;
        node.next = P::null();

        if self.first.is_null() {
            self.first = node;
            self.last = self.first;
        } else {
            self.last.next = node;
            self.last = node;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.first.is_null()
    }

    pub fn first(&self) -> Option<T> {
        if !self.first.is_null() {
            return Some(self.first.val);
        }
        None
    }
}

pub struct ListIterator<T: Copy> {
    curr: P<Node<T>>,
}

impl<T: Copy> List<T> {
    pub fn iter(self) -> ListIterator<T> {
        ListIterator { curr: self.first }
    }
}

impl<T: Copy> Iterator for ListIterator<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            return None;
        }
        let val = self.curr.val;
        self.curr = self.curr.next;
        return Some(val);
    }
}
