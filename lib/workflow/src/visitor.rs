//  VISITOR.rs
//    by Lut99
//
//  Created:
//    08 Oct 2024, 17:35:03
//  Last edited:
//    14 Oct 2024, 11:55:45
//  Auto updated?
//    Yes
//
//  Description:
//!   Implements a visitor for [`Workflow`] elements.
//

use std::error::Error;

use crate::{Elem, ElemBranch, ElemCall, ElemLoop, ElemParallel, Workflow};


/***** LIBRARY *****/
/// Visits [`Elem`]s by reference.
pub trait Visitor<'w> {
    /// The error returned by this Visitor.
    ///
    /// Tip: use [`Infallible`](std::convert::Infallible) if you don't have any.
    type Error: Error;


    /// Visits an [`Elem`].
    ///
    /// The default implementation determines what node it deals with, and then calls the
    /// appropriate `Visitor::visit_X()`-function. It's usually not necessary to override this
    /// method.
    ///
    /// # Arguments
    /// - `elem`: The visited [`Elem`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    fn visit(&mut self, elem: &'w Elem) -> Result<(), Self::Error> {
        match elem {
            Elem::Call(c) => self.visit_call(c),

            Elem::Branch(b) => self.visit_branch(b),
            Elem::Parallel(p) => self.visit_parallel(p),
            Elem::Loop(l) => self.visit_loop(l),

            Elem::Next => self.visit_next(),
            Elem::Stop => self.visit_stop(),
        }
    }


    /// Visits an [`Elem::Call`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemCall`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_call(&mut self, elem: &'w ElemCall) -> Result<(), Self::Error> { self.visit(&elem.next) }

    /// Visits an [`Elem::Branch`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemBranch`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_branch(&mut self, elem: &'w ElemBranch) -> Result<(), Self::Error> {
        for b in &elem.branches {
            self.visit(b)?;
        }
        self.visit(&elem.next)
    }

    /// Visits an [`Elem::Parallel`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemParallel`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_parallel(&mut self, elem: &'w ElemParallel) -> Result<(), Self::Error> {
        for b in &elem.branches {
            self.visit(b)?;
        }
        self.visit(&elem.next)
    }

    /// Visits an [`Elem::Loop`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the body, and
    /// then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemLoop`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_loop(&mut self, elem: &'w ElemLoop) -> Result<(), Self::Error> {
        self.visit(&elem.body)?;
        self.visit(&elem.next)
    }


    /// Visits an [`Elem::Next`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_next(&mut self) -> Result<(), Self::Error> { Ok(()) }

    /// Visits an [`Elem::Stop`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_stop(&mut self) -> Result<(), Self::Error> { Ok(()) }
}
impl<'a, 'w, T: Visitor<'w>> Visitor<'w> for &'a mut T {
    type Error = T::Error;

    #[inline]
    fn visit(&mut self, elem: &'w Elem) -> Result<(), Self::Error> { T::visit(self, elem) }

    #[inline]
    fn visit_call(&mut self, elem: &'w ElemCall) -> Result<(), Self::Error> { T::visit_call(self, elem) }

    #[inline]
    fn visit_branch(&mut self, elem: &'w ElemBranch) -> Result<(), Self::Error> { T::visit_branch(self, elem) }

    #[inline]
    fn visit_parallel(&mut self, elem: &'w ElemParallel) -> Result<(), Self::Error> { T::visit_parallel(self, elem) }

    #[inline]
    fn visit_loop(&mut self, elem: &'w ElemLoop) -> Result<(), Self::Error> { T::visit_loop(self, elem) }

    #[inline]
    fn visit_next(&mut self) -> Result<(), Self::Error> { T::visit_next(self) }

    #[inline]
    fn visit_stop(&mut self) -> Result<(), Self::Error> { T::visit_stop(self) }
}



/// Visits [`Elem`]s by mutable reference.
pub trait VisitorMut<'w> {
    /// The error returned by this VisitorMut.
    ///
    /// Tip: use [`Infallible`](std::convert::Infallible) if you don't have any.
    type Error: Error;


    /// Visits an [`Elem`].
    ///
    /// The default implementation determines what node it deals with, and then calls the
    /// appropriate `Visitor::visit_X()`-function. It's usually not necessary to override this
    /// method.
    ///
    /// # Arguments
    /// - `elem`: The visited [`Elem`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    fn visit(&mut self, elem: &'w mut Elem) -> Result<(), Self::Error> {
        match elem {
            Elem::Call(c) => self.visit_call(c),

            Elem::Branch(b) => self.visit_branch(b),
            Elem::Parallel(p) => self.visit_parallel(p),
            Elem::Loop(l) => self.visit_loop(l),

            Elem::Next => self.visit_next(),
            Elem::Stop => self.visit_stop(),
        }
    }


    /// Visits an [`Elem::Call`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemCall`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_call(&mut self, elem: &'w mut ElemCall) -> Result<(), Self::Error> { self.visit(&mut elem.next) }

    /// Visits an [`Elem::Branch`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemBranch`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_branch(&mut self, elem: &'w mut ElemBranch) -> Result<(), Self::Error> {
        for b in &mut elem.branches {
            self.visit(b)?;
        }
        self.visit(&mut elem.next)
    }

    /// Visits an [`Elem::Parallel`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemParallel`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_parallel(&mut self, elem: &'w mut ElemParallel) -> Result<(), Self::Error> {
        for b in &mut elem.branches {
            self.visit(b)?;
        }
        self.visit(&mut elem.next)
    }

    /// Visits an [`Elem::Loop`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the body, and
    /// then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemLoop`].
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_loop(&mut self, elem: &'w mut ElemLoop) -> Result<(), Self::Error> {
        self.visit(&mut elem.body)?;
        self.visit(&mut elem.next)
    }


    /// Visits an [`Elem::Next`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_next(&mut self) -> Result<(), Self::Error> { Ok(()) }

    /// Visits an [`Elem::Stop`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_stop(&mut self) -> Result<(), Self::Error> { Ok(()) }
}
impl<'a, 'w, T: VisitorMut<'w>> VisitorMut<'w> for &'a mut T {
    type Error = T::Error;

    #[inline]
    fn visit(&mut self, elem: &'w mut Elem) -> Result<(), Self::Error> { T::visit(self, elem) }

    #[inline]
    fn visit_call(&mut self, elem: &'w mut ElemCall) -> Result<(), Self::Error> { T::visit_call(self, elem) }

    #[inline]
    fn visit_branch(&mut self, elem: &'w mut ElemBranch) -> Result<(), Self::Error> { T::visit_branch(self, elem) }

    #[inline]
    fn visit_parallel(&mut self, elem: &'w mut ElemParallel) -> Result<(), Self::Error> { T::visit_parallel(self, elem) }

    #[inline]
    fn visit_loop(&mut self, elem: &'w mut ElemLoop) -> Result<(), Self::Error> { T::visit_loop(self, elem) }

    #[inline]
    fn visit_next(&mut self) -> Result<(), Self::Error> { T::visit_next(self) }

    #[inline]
    fn visit_stop(&mut self) -> Result<(), Self::Error> { T::visit_stop(self) }
}



/// Visits [`Elem`]s by ownership.
pub trait VisitorOwned {
    /// The error returned by this VisitorMut.
    ///
    /// Tip: use [`Infallible`](std::convert::Infallible) if you don't have any.
    type Error: Error;


    /// Visits an [`Elem`].
    ///
    /// The default implementation determines what node it deals with, and then calls the
    /// appropriate `Visitor::visit_X()`-function. It's usually not necessary to override this
    /// method.
    ///
    /// # Arguments
    /// - `elem`: The visited [`Elem`].
    ///
    /// # Returns
    /// The element returned by the appropriate visit.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    fn visit(&mut self, elem: Elem) -> Result<Elem, Self::Error> {
        match elem {
            Elem::Call(c) => self.visit_call(c),

            Elem::Branch(b) => self.visit_branch(b),
            Elem::Parallel(p) => self.visit_parallel(p),
            Elem::Loop(l) => self.visit_loop(l),

            Elem::Next => self.visit_next(),
            Elem::Stop => self.visit_stop(),
        }
    }

    /// Visits an [`Elem`] that is only accessible through a mutable reference.
    ///
    /// This wraps [`VisitorOwned::visit()`] and deals with the mutability.
    ///
    /// # Arguments
    /// - `elem`: The visited [`Elem`]. Note that it's automatically updated if the visit lead that
    ///   way.
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    fn visit_mut(&mut self, elem: &mut Elem) -> Result<(), Self::Error> {
        let mut temp: Elem = Elem::Stop;
        std::mem::swap(&mut temp, elem);
        *elem = self.visit(temp)?;
        Ok(())
    }


    /// Visits an [`Elem::Call`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemCall`].
    ///
    /// # Returns
    /// The given `elem` (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_call(&mut self, mut elem: ElemCall) -> Result<Elem, Self::Error> {
        self.visit_mut(&mut elem.next)?;
        Ok(Elem::Call(elem))
    }

    /// Visits an [`Elem::Branch`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemBranch`].
    ///
    /// # Returns
    /// The given `elem` (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_branch(&mut self, mut elem: ElemBranch) -> Result<Elem, Self::Error> {
        for b in &mut elem.branches {
            self.visit_mut(b)?;
        }
        self.visit_mut(&mut elem.next)?;
        Ok(Elem::Branch(elem))
    }

    /// Visits an [`Elem::Parallel`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the branches,
    /// and then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemParallel`].
    ///
    /// # Returns
    /// The given `elem` (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_parallel(&mut self, mut elem: ElemParallel) -> Result<Elem, Self::Error> {
        for b in &mut elem.branches {
            self.visit_mut(b)?;
        }
        self.visit_mut(&mut elem.next)?;
        Ok(Elem::Parallel(elem))
    }

    /// Visits an [`Elem::Loop`].
    ///
    /// The default implementation doesn't do anything meaningful besides visiting the body, and
    /// then the next node.
    ///
    /// # Arguments
    /// - `elem`: The visited [`ElemLoop`].
    ///
    /// # Returns
    /// The given `elem` (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_loop(&mut self, mut elem: ElemLoop) -> Result<Elem, Self::Error> {
        self.visit_mut(&mut elem.body)?;
        self.visit_mut(&mut elem.next)?;
        Ok(Elem::Loop(elem))
    }


    /// Visits an [`Elem::Next`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Returns
    /// An [`Elem::Next`] (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_next(&mut self) -> Result<Elem, Self::Error> { Ok(Elem::Next) }

    /// Visits an [`Elem::Stop`].
    ///
    /// The default implementation doesn't do anything meaningful.
    ///
    /// # Returns
    /// An [`Elem::Stop`] (i.e., nothing is replaced).
    ///
    /// # Errors
    /// If this visitor fails, then the whole visiting processes is terminated.
    #[inline]
    fn visit_stop(&mut self) -> Result<Elem, Self::Error> { Ok(Elem::Stop) }
}
impl<'a, T: VisitorOwned> VisitorOwned for &'a mut T {
    type Error = T::Error;

    #[inline]
    fn visit(&mut self, elem: Elem) -> Result<Elem, Self::Error> { T::visit(self, elem) }

    #[inline]
    fn visit_call(&mut self, elem: ElemCall) -> Result<Elem, Self::Error> { T::visit_call(self, elem) }

    #[inline]
    fn visit_branch(&mut self, elem: ElemBranch) -> Result<Elem, Self::Error> { T::visit_branch(self, elem) }

    #[inline]
    fn visit_parallel(&mut self, elem: ElemParallel) -> Result<Elem, Self::Error> { T::visit_parallel(self, elem) }

    #[inline]
    fn visit_loop(&mut self, elem: ElemLoop) -> Result<Elem, Self::Error> { T::visit_loop(self, elem) }

    #[inline]
    fn visit_next(&mut self) -> Result<Elem, Self::Error> { T::visit_next(self) }

    #[inline]
    fn visit_stop(&mut self) -> Result<Elem, Self::Error> { T::visit_stop(self) }
}





/***** IMPLEMENTATIONS *****/
// Visitor
impl Workflow {
    /// Visits all [`Elem`]ents in this graph and calls the appropriate functions for them.
    ///
    /// # Arguments
    /// - `visitor`: The [`Visitor`] used to visit all elements.
    #[inline]
    pub fn visit<'w, V: Visitor<'w>>(&'w self, mut visitor: V) -> Result<(), V::Error> { visitor.visit(&self.start) }

    /// Visits all [`Elem`]ents in this graph mutably and calls the appropriate functions for them.
    ///
    /// # Arguments
    /// - `visitor`: The [`Visitor`] used to visit all elements.
    #[inline]
    pub fn visit_mut<'w, V: VisitorMut<'w>>(&'w mut self, mut visitor: V) -> Result<(), V::Error> { visitor.visit(&mut self.start) }

    /// Visits all [`Elem`]ents in this graph by ownership and calls the appropriate functions for them.
    ///
    /// # Arguments
    /// - `visitor`: The [`Visitor`] used to visit all elements.
    #[inline]
    pub fn visit_owned<V: VisitorOwned>(&mut self, mut visitor: V) -> Result<(), V::Error> { visitor.visit_mut(&mut self.start) }
}
