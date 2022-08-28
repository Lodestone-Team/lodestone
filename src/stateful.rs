use crate::traits::Error;

pub struct Stateful<T> {
    inner: T,
    on_update: Box<dyn Fn(&T, &T) -> Result<(), Error> + Send + Sync>,
    on_transform: Box<dyn Fn(&T, &T) -> Result<(), Error> + Send + Sync>,
}

impl<T> Stateful<T> {
    pub fn new(
        inner: T,
        on_update: Box<dyn Fn(&T, &T) -> Result<(), Error> + Send + Sync>,
        on_transform: Box<dyn Fn(&T, &T) -> Result<(), Error> + Send + Sync>,
    ) -> Stateful<T> {
        Stateful {
            inner,
            on_update,
            on_transform,
        }
    }
    pub fn update(&mut self, inner: T) -> Result<(), Error> {
        (self.on_update)(&self.inner, &inner)?;
        self.inner = inner;
        Ok(())
    }
    pub fn transform(
        &mut self,
        mut update: Box<dyn FnMut(&mut T) -> Result<(), Error>>,
    ) -> Result<(), Error> {
        update(&mut self.inner)?;
        (self.on_transform)(&self.inner, &self.inner)?;
        Ok(())
    }
    pub fn get_ref(&self) -> &T {
        &self.inner
    }
}

impl<T> Stateful<T>
where
    T: Clone,
{
    pub fn get(&self) -> T {
        self.inner.clone()
    }
    pub fn transform_cmp(&mut self, mut update: Box<dyn FnMut(&mut T)>) -> Result<(), Error> {
        let old = self.inner.clone();
        update(&mut self.inner);
        (self.on_transform)(&self.inner, &old)
    }
}
