use crate::traits::Error;

pub struct Stateful<T, A> {
    inner: T,
    on_update: Box<dyn Fn(&T, &T, &A) -> Result<(), Error> + Send + Sync>,
    on_transform: Box<dyn Fn(&T, &T, &A) -> Result<(), Error> + Send + Sync>,
}

impl<T,A> Stateful<T,A> {
    pub fn new(
        inner: T,
        on_update: Box<dyn Fn(&T, &T, &A) -> Result<(), Error> + Send + Sync>,
        on_transform: Box<dyn Fn(&T, &T, &A) -> Result<(), Error> + Send + Sync>,
    ) -> Stateful<T, A> {
        Stateful {
            inner,
            on_update,
            on_transform,
        }
    }
    pub fn update(&mut self, inner: T, aux : A) -> Result<(), Error> {
        (self.on_update)(&self.inner, &inner, &aux)?;
        self.inner = inner;
        Ok(())
    }
    pub fn transform(
        &mut self,
        mut update: Box<dyn FnMut(&mut T) -> Result<(), Error>>,
        aux : A
    ) -> Result<(), Error> {
        update(&mut self.inner)?;
        (self.on_transform)(&self.inner, &self.inner, &aux)
    }
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
}

impl<T, A> Stateful<T, A>
where
    T: Clone,
{
    pub fn get(&self) -> T {
        self.inner.clone()
    }
    pub fn transform_cmp(
        &mut self,
        mut update: Box<dyn FnMut(&mut T) -> Result<(), Error>>,
        aux : A
    ) -> Result<(), Error> {
        let old = self.inner.clone();
        update(&mut self.inner)?;
        (self.on_transform)(&self.inner, &old, &aux)
    }
}
