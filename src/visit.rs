//! Value traversal traits.

// Copyright 2020 Google Inc. All Rights Reserved.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

#![warn(missing_docs)]

use crate::io::{DecodeError, PathItem};
use std::convert::Infallible;
use thiserror::Error;
pub(crate) use wasmbin_derive::Visit;

/// Error type for [Visit] traversals.
#[derive(Error, Debug)]
pub enum VisitError<E> {
    /// Decoding error occured while visiting a [`Lazy`](crate::builtins::Lazy) value.
    #[error(transparent)]
    LazyDecode(DecodeError),

    /// A custom error returned from a visitor callback.
    #[error(transparent)]
    Custom(E),
}

impl<E> VisitError<E> {
    pub(crate) fn in_path(self, item: PathItem) -> Self {
        match self {
            VisitError::LazyDecode(err) => VisitError::LazyDecode(err.in_path(item)),
            err @ VisitError::Custom(_) => err,
        }
    }
}

impl From<VisitError<Infallible>> for DecodeError {
    fn from(err: VisitError<Infallible>) -> Self {
        match err {
            VisitError::Custom(err) => match err {},
            VisitError::LazyDecode(err) => err,
        }
    }
}

/// A trait for results that can be returned from a visitor.
pub trait VisitResult {
    /// The error type of the result.
    type Error;

    /// Convert this result into a standard [`Result`].
    ///
    /// # Errors
    ///
    /// Returns the visitor's error, or `()` if a boolean visitor returned `false`.
    fn into_result(self) -> Result<(), Self::Error>;
}

impl VisitResult for () {
    type Error = Infallible;

    fn into_result(self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl VisitResult for bool {
    type Error = ();

    fn into_result(self) -> Result<(), Self::Error> {
        if self { Ok(()) } else { Err(()) }
    }
}

impl<E> VisitResult for Result<(), E> {
    type Error = E;

    fn into_result(self) -> Result<(), Self::Error> {
        self
    }
}

/// A trait for recursively finding instances of a given type within a value.
///
/// This is particularly useful for finding and updating [indices](crate::indices).
pub trait Visit: 'static + Sized {
    /// Traverse this value with the provided callback.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding a lazy value fails or the callback reports a failure.
    fn visit<'a, T: 'static, R: VisitResult, F: FnMut(&'a T) -> R>(
        &'a self,
        mut f: F,
    ) -> Result<(), VisitError<R::Error>> {
        self.visit_child(&mut move |item| f(item).into_result())
    }

    /// Traverse this value mutably with the provided callback.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding a lazy value fails or the callback reports a failure.
    fn visit_mut<T: 'static, R: VisitResult, F: FnMut(&mut T) -> R>(
        &mut self,
        mut f: F,
    ) -> Result<(), VisitError<R::Error>> {
        self.visit_child_mut(&mut move |item| f(item).into_result())
    }

    #[doc(hidden)]
    /// Internal implementation of [`visit`](Visit::visit).
    ///
    /// Takes a mutable reference to the callback to avoid infinite
    /// generic recursion.
    fn visit_child<'a, T: 'static, E, F: FnMut(&'a T) -> Result<(), E>>(
        &'a self,
        f: &mut F,
    ) -> Result<(), VisitError<E>> {
        if let Some(v) = <dyn std::any::Any>::downcast_ref(self) {
            f(v).map_err(VisitError::Custom)?;
        }
        self.visit_children(f)
    }

    #[doc(hidden)]
    /// Internal implementation of [`visit_mut`](Visit::visit_mut).
    ///
    /// Takes a mutable reference to the callback to avoid infinite
    /// generic recursion.
    fn visit_child_mut<T: 'static, E, F: FnMut(&mut T) -> Result<(), E>>(
        &mut self,
        f: &mut F,
    ) -> Result<(), VisitError<E>> {
        if let Some(v) = <dyn std::any::Any>::downcast_mut(self) {
            f(v).map_err(VisitError::Custom)?;
        }
        self.visit_children_mut(f)
    }

    /// Traverse the children of this value with the provided callback.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding a lazy child fails or the callback reports a failure.
    fn visit_children<'a, T: 'static, E, F: FnMut(&'a T) -> Result<(), E>>(
        &'a self,
        _f: &mut F,
    ) -> Result<(), VisitError<E>> {
        Ok(())
    }

    /// Traverse the children of this value mutably with the provided callback.
    ///
    /// # Errors
    ///
    /// Returns an error if decoding a lazy child fails or the callback reports a failure.
    fn visit_children_mut<T: 'static, E, F: FnMut(&mut T) -> Result<(), E>>(
        &mut self,
        _f: &mut F,
    ) -> Result<(), VisitError<E>> {
        Ok(())
    }
}
