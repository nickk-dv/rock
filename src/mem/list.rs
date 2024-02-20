use super::{arena::Arena, ptr::P};
use std::marker::PhantomData;

#[derive(Copy, Clone)]
pub struct ListBuilder<T: Copy> {
    first: P<Node<T>>,
    last: P<Node<T>>,
}

#[derive(Copy, Clone)]
pub struct List<T: Copy> {
    first: P<Node<T>>,
}

#[derive(Copy, Clone)]
struct Node<T: Copy> {
    val: T,
    next: P<Node<T>>,
}

impl<T: Copy> ListBuilder<T> {
    pub fn new() -> Self {
        Self {
            first: P::null(),
            last: P::null(),
        }
    }

    pub fn take(self) -> List<T> {
        List::<T> { first: self.first }
    }

    pub fn add(&mut self, arena: &mut Arena, val: T) {
        let mut node = arena.alloc::<Node<T>>();
        node.val = val;
        node.next = P::null();

        if self.first.is_null() {
            self.first = node;
            self.last = node;
        } else {
            self.last.next = node;
            self.last = node;
        }
    }
}

impl<T: Copy> List<T> {
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    pub fn is_empty(&self) -> bool {
        self.first.is_null()
    }

    pub fn iter(&self) -> ListIter<T> {
        ListIter {
            curr: self.first,
            phantom: PhantomData,
        }
    }

    pub fn iter_mut(&self) -> ListIterMut<T> {
        ListIterMut {
            curr: self.first,
            phantom: PhantomData,
        }
    }

    pub fn iter_last(&self) -> ListIterValLast<T> {
        ListIterValLast { curr: self.first }
    }
}

pub struct ListIter<'a, T: Copy> {
    curr: P<Node<T>>,
    phantom: PhantomData<&'a T>,
}

pub struct ListIterMut<'a, T: Copy> {
    curr: P<Node<T>>,
    phantom: PhantomData<&'a T>,
}

pub struct ListIterVal<T: Copy> {
    curr: P<Node<T>>,
}

pub struct ListIterValLast<T: Copy> {
    curr: P<Node<T>>,
}

impl<'a, T: Copy> Iterator for ListIter<'a, T> {
    type Item = &'a T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let val = unsafe { &(*self.curr.as_mut()).val };
            self.curr = self.curr.next;
            Some(val)
        }
    }
}

impl<'a, T: Copy> Iterator for ListIterMut<'a, T> {
    type Item = &'a mut T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let val = unsafe { &mut (*self.curr.as_mut()).val };
            self.curr = self.curr.next;
            Some(val)
        }
    }
}

impl<T: Copy> Iterator for ListIterVal<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let val = self.curr.val;
            self.curr = self.curr.next;
            Some(val)
        }
    }
}

impl<T: Copy> Iterator for ListIterValLast<T> {
    type Item = (T, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.curr.is_null() {
            None
        } else {
            let val = self.curr.val;
            self.curr = self.curr.next;
            let last = self.curr.is_null();
            Some((val, last))
        }
    }
}

impl<T: Copy> IntoIterator for List<T> {
    type Item = T;

    type IntoIter = ListIterVal<T>;

    fn into_iter(self) -> Self::IntoIter {
        ListIterVal { curr: self.first }
    }
}
