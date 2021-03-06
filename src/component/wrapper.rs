//! Wrapper for components with shared state.
use std::collections::HashSet;
use std::rc::Rc;

use yew::{
    agent::{Agent, AgentLink, Bridge, Bridged, Context, HandlerId},
    prelude::*,
};

use crate::handle::{Handle, SharedState};
use crate::handler::{Handler, Reduction, ReductionOnce};

enum Request<T> {
    /// Apply a state change.
    Apply(Reduction<T>),
    /// Apply a state change once.
    ApplyOnce(ReductionOnce<T>),
}

enum Response<T> {
    /// Update subscribers with current state.
    State(Rc<T>),
}

/// Context agent for managing shared state. In charge of applying changes to state then notifying
/// subscribers of new state.
struct SharedStateService<T, SCOPE>
where
    T: Handler + Clone + 'static,
    SCOPE: 'static,
{
    handler: T,
    subscriptions: HashSet<HandlerId>,
    link: AgentLink<SharedStateService<T, SCOPE>>,
}

impl<T, SCOPE> Agent for SharedStateService<T, SCOPE>
where
    T: Handler + Clone + 'static,
    SCOPE: 'static,
{
    type Message = ();
    type Reach = Context<Self>;
    type Input = Request<<T as Handler>::Model>;
    type Output = Response<<T as Handler>::Model>;

    fn create(link: AgentLink<Self>) -> Self {
        Self {
            handler: <T as Handler>::new(),
            subscriptions: Default::default(),
            link,
        }
    }

    fn update(&mut self, _msg: Self::Message) {}

    fn handle_input(&mut self, msg: Self::Input, _who: HandlerId) {
        match msg {
            Request::Apply(reduce) => {
                self.handler.apply(reduce);
            }
            Request::ApplyOnce(reduce) => {
                self.handler.apply_once(reduce);
            }
        }

        // Notify subscribers of change
        for who in self.subscriptions.iter().cloned() {
            self.link
                .respond(who, Response::State(self.handler.state()));
        }
    }

    fn connected(&mut self, who: HandlerId) {
        self.subscriptions.insert(who);
        self.link
            .respond(who, Response::State(self.handler.state()));
    }

    fn disconnected(&mut self, who: HandlerId) {
        self.subscriptions.remove(&who);
    }
}

type StateHandler<T> = <<T as SharedState>::Handle as Handle>::Handler;
type Model<T> = <StateHandler<T> as Handler>::Model;

/// Component wrapper for managing messages and state handles.
///
/// Wraps any component with properties that implement `SharedState`:
/// ```
/// pub type MyComponent = SharedStateComponent<MyComponentModel>;
/// ```
///
/// A scope may be provided to specify where the state is shared:
/// ```
/// // This will only share state with other components using `FooScope`.
/// pub struct FooScope;
/// pub type MyComponent = SharedStateComponent<MyComponentModel, FooScope>;
/// ```
///
/// # Important
/// By default `StorageHandle` and `GlobalHandle` have different scopes. Though not enforced,
/// components with different handles should not use the same scope.
pub struct SharedStateComponent<C, SCOPE = StateHandler<<C as Component>::Properties>>
where
    C: Component,
    C::Properties: SharedState + Clone,
    StateHandler<C::Properties>: Clone,
    SCOPE: 'static,
{
    props: C::Properties,
    bridge: Box<dyn Bridge<SharedStateService<StateHandler<C::Properties>, SCOPE>>>,
}

#[doc(hidden)]
pub enum SharedStateComponentMsg<T> {
    /// Recieve new local state.
    /// IMPORTANT: Changes will **not** be reflected in shared state.
    SetLocal(Rc<T>),
    /// Update shared state.
    Apply(Reduction<T>),
    ApplyOnce(ReductionOnce<T>),
}

impl<C, SCOPE> Component for SharedStateComponent<C, SCOPE>
where
    C: Component,
    C::Properties: SharedState + Clone,
    Model<C::Properties>: Default,
    StateHandler<C::Properties>: Clone,
{
    type Message = SharedStateComponentMsg<Model<C::Properties>>;
    type Properties = C::Properties;

    fn create(mut props: Self::Properties, link: ComponentLink<Self>) -> Self {
        use SharedStateComponentMsg::*;
        // Bridge to receive new state.
        let callback = link.callback(|msg| match msg {
            Response::State(state) => SetLocal(state),
        });
        let bridge = SharedStateService::bridge(callback);

        props
            .handle()
            .set_local_callback(link.callback(Apply), link.callback(ApplyOnce));

        SharedStateComponent { props, bridge }
    }

    fn update(&mut self, msg: Self::Message) -> ShouldRender {
        use SharedStateComponentMsg::*;
        match msg {
            Apply(reduce) => {
                self.bridge.send(Request::Apply(reduce));
                false
            }
            ApplyOnce(reduce) => {
                self.bridge.send(Request::ApplyOnce(reduce));
                false
            }
            SetLocal(state) => {
                self.props.handle().set_local_state(state);
                true
            }
        }
    }

    fn change(&mut self, mut props: Self::Properties) -> ShouldRender {
        props.handle().set_local(self.props.handle());
        self.props = props;
        true
    }

    fn view(&self) -> Html {
        let props = self.props.clone();
        html! {
            <C with props />
        }
    }
}
