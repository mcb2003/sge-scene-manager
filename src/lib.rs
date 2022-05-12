use std::error::Error;

pub trait Scene {
    type Context;

    fn on_enter(&mut self, _ctx: &mut Self::Context) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn on_leave(&mut self, _ctx: &mut Self::Context) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn on_pause(
        &mut self,
        _ctx: &mut Self::Context,
        _should_draw: bool,
    ) -> Result<(), Box<dyn Error>> {
        Ok(())
    }
    fn on_unpause(&mut self, _ctx: &mut Self::Context) -> Result<(), Box<dyn Error>> {
        Ok(())
    }

    fn on_update(
        &mut self,
        ctx: &mut Self::Context,
        elapsed_time: f64,
    ) -> Result<Transition<Self::Context>, Box<dyn Error>>;

    fn on_event(
        &mut self,
        _ctx: &mut Self::Context,
        _event: &sge::Event,
    ) -> Result<(bool, Transition<Self::Context>), Box<dyn Error>> {
        Ok((false, Transition::None))
    }

    fn draw_next(&self, _ctx: &mut Self::Context) -> bool {
        false
    }
}

pub enum Transition<C> {
    None,
    Push(Box<dyn Scene<Context = C>>),
    Pop,
    Replace(Box<dyn Scene<Context = C>>),
}

impl<C> Transition<C> {
    pub fn push(s: impl Scene<Context = C> + 'static) -> Self {
        Self::Push(Box::new(s))
    }

    pub fn replace(s: impl Scene<Context = C> + 'static) -> Self {
        Self::Replace(Box::new(s))
    }

    fn apply_to(
        self,
        scenes: &mut Vec<Box<dyn Scene<Context = C>>>,
        ctx: &mut C,
    ) -> Result<(), Box<dyn Error>> {
        match self {
            Transition::Push(mut new) => {
                if let Some(last) = scenes.last_mut() {
                    let draw_next = new.draw_next(ctx);
                    last.on_pause(ctx, draw_next)?;
                }
                new.on_enter(ctx)?;
                scenes.push(new);
            }
            Transition::Pop => {
                if let Some(mut old) = scenes.pop() {
                    old.on_leave(ctx)?;
                }
                if let Some(last) = scenes.last_mut() {
                    last.on_unpause(ctx)?;
                }
            }
            Transition::Replace(mut new) => {
                let last = scenes
                    .last_mut()
                    .expect("Tried to replace a scene that does not exist");

                let draw_next = new.draw_next(ctx);
                last.on_pause(ctx, draw_next)?;

                new.on_enter(ctx)?;
                let mut old = std::mem::replace(last, new);
                old.on_leave(ctx)?;
            }
            _ => {}
        }
        Ok(())
    }
}

pub struct SceneManager<C> {
    scenes: Vec<Box<dyn Scene<Context = C>>>,
    operations: Vec<Transition<C>>,
    /// The context, passed to scenes each loop iteration
    pub ctx: C,
}

impl<C> SceneManager<C> {
    pub fn new(ctx: C) -> Self {
        Self {
            ctx,
            scenes: Vec::new(),
            // Kept around to avoid allocating on every frame
            operations: Vec::new(),
        }
    }

    pub fn apply(&mut self, trans: Transition<C>) -> Result<(), Box<dyn Error>> {
        trans.apply_to(&mut self.scenes, &mut self.ctx)
    }

    pub fn push(&mut self, mut new: Box<dyn Scene<Context = C>>) -> Result<(), Box<dyn Error>> {
        new.on_enter(&mut self.ctx)?;
        self.scenes.push(new);
        Ok(())
    }

    pub fn pop(&mut self) -> Result<Option<Box<dyn Scene<Context = C>>>, Box<dyn Error>> {
        let mut old = self.scenes.pop();
        if let Some(ref mut old) = old {
            old.on_leave(&mut self.ctx)?;
        }
        Ok(old)
    }

    pub fn replace(
        &mut self,
        mut new: Box<dyn Scene<Context = C>>,
    ) -> Result<Box<dyn Scene<Context = C>>, Box<dyn Error>> {
        let last = self
            .scenes
            .last_mut()
            .expect("Tried to replace a scene that does not exist");
        new.on_enter(&mut self.ctx)?;
        let mut old = std::mem::replace(last, new);
        old.on_leave(&mut self.ctx)?;
        Ok(old)
    }
}

impl<C> sge::Application for SceneManager<C> {
    fn on_create(&mut self) -> sge::ApplicationResult {
        // If there are no scenes, quit
        Ok(!self.scenes.is_empty())
    }

    fn on_update(&mut self, elapsed_time: f64) -> sge::ApplicationResult {
        for scene in self.scenes.iter_mut().rev() {
            let trans = scene.on_update(&mut self.ctx, elapsed_time)?;
            self.operations.push(trans);
            if !scene.draw_next(&mut self.ctx) {
                break;
            }
        }
        for op in self.operations.drain(..) {
            op.apply_to(&mut self.scenes, &mut self.ctx)?;
        }
        // If there are no more scenes, quit
        Ok(!self.scenes.is_empty())
    }

    fn on_event(&mut self, event: &sge::Event) -> sge::ApplicationResult {
        let mut was_handled = false;
        for scene in self.scenes.iter_mut().rev() {
            let (handled, trans) = scene.on_event(&mut self.ctx, event)?;
            was_handled |= handled;
            self.operations.push(trans);
            if handled || !scene.draw_next(&mut self.ctx) {
                break;
            }
        }
        for op in self.operations.drain(..) {
            op.apply_to(&mut self.scenes, &mut self.ctx)?;
        }
        Ok(was_handled)
    }
}
