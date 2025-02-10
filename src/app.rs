use yew::prelude::*;
use gloo::timers::callback::Interval;
use num_bigint::BigUint;
use num_traits::{Zero, One};
use std::rc::Rc;
use web_sys::console;
use yew::Reducible;
use gloo::storage::{LocalStorage, Storage};
use serde::{Serialize, Deserialize};

const SUFFIXES: &[&str] = &["", "K", "M", "B", "T", "Qa", "Qi", "Sx", "Sp", "Oc"];

fn format_number(num: &BigUint) -> String {
    let num_str = num.to_string();
    let len = num_str.len();
    
    if len <= 3 {
        return num_str;
    }
    
    if len / 3 >= SUFFIXES.len() {
        // Use scientific notation for numbers beyond our suffix list
        let first_digit = &num_str[..1];
        let second_digits = num_str.get(1..3).unwrap_or("0");
        return format!("{}.{}e{}", first_digit, second_digits, len - 1);
    }
    
    let suffix_index = (len - 1) / 3;
    let offset = len - (suffix_index * 3);
    
    let main_digits = &num_str[..offset];
    let decimal_digits = num_str.get(offset..offset + 2).unwrap_or("00");
    
    if decimal_digits == "00" {
        format!("{}{}", main_digits, SUFFIXES[suffix_index])
    } else {
        format!("{}.{}{}", main_digits, decimal_digits, SUFFIXES[suffix_index])
    }
}

mod big_uint_serde {
    use super::*;
    use serde::{Serializer, Deserializer};

    pub fn serialize<S>(num: &BigUint, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        num.to_string().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BigUint, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct State {
    #[serde(with = "big_uint_serde")]
    counter: BigUint,
    #[serde(with = "big_uint_serde")]
    production: BigUint,
    last_save: f64,
    last_saved_at: Option<f64>,
}

#[derive(Clone)]
pub enum Msg {
    Tick,
    UpgradeProduction,
    Save,
    Load,
    Reset,
}

fn reducer(state: &State, msg: Msg) -> State {
    match msg {
        Msg::Tick => {
            // Update counter by adding production.
            let new_counter = state.counter.clone() + state.production.clone();
            State {
                counter: new_counter,
                production: state.production.clone(),
                last_save: state.last_save,
                last_saved_at: state.last_saved_at,
            }
        }
        Msg::UpgradeProduction => {
            // Double the production value.
            let new_production = state.production.clone() * 2u32;
            State {
                counter: state.counter.clone(),
                production: new_production,
                last_save: state.last_save,
                last_saved_at: state.last_saved_at,
            }
        }
        Msg::Save => {
            state.save().unwrap_or_else(|e| console::log_1(&e.into()));
            state.clone()
        }
        Msg::Load => {
            State::load().unwrap_or_else(|| state.clone())
        }
        Msg::Reset => State::new(),
    }
}

impl Reducible for State {
    type Action = Msg;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        Rc::new(reducer(&self, action))
    }
}

impl State {
    fn new() -> Self {
        Self {
            counter: BigUint::zero(),
            production: BigUint::one(),
            last_save: js_sys::Date::now(),
            last_saved_at: None,
        }
    }

    fn save(&self) -> Result<(), String> {
        let mut state = self.clone();
        state.last_saved_at = Some(js_sys::Date::now());
        LocalStorage::set("idle_game_save", &state).map_err(|e| e.to_string())
    }

    fn load() -> Option<Self> {
        LocalStorage::get("idle_game_save").ok().map(|mut state: State| {
            // Calculate offline progress
            let now = js_sys::Date::now();
            let elapsed_seconds = ((now - state.last_save) / 1000.0) as u32;
            if elapsed_seconds > 0 {
                state.counter += &state.production * elapsed_seconds;
            }
            state.last_save = now;
            state
        })
    }

    fn format_last_saved(&self) -> String {
        self.last_saved_at.map_or("Never".to_string(), |timestamp| {
            let seconds_ago = (js_sys::Date::now() - timestamp) / 1000.0;
            if seconds_ago < 60.0 {
                "Just now".to_string()
            } else if seconds_ago < 3600.0 {
                format!("{:.0} minutes ago", seconds_ago / 60.0)
            } else {
                format!("{:.1} hours ago", seconds_ago / 3600.0)
            }
        })
    }
}

#[function_component(App)]
pub fn app() -> Html {
    let state = use_reducer(|| State::load().unwrap_or_else(State::new));
    let interval_key = use_state(|| 0);
    let time_update = use_state(|| 0);

    // Add this effect for updating the time display
    {
        let time_update = time_update.clone();
        use_effect_with_deps(
            move |_| {
                let interval = Interval::new(1000, move || {
                    time_update.set(*time_update + 1);
                });
                move || drop(interval)
            },
            (),
        );
    }

    // Setup autosave interval
    {
        let state = state.clone();
        use_effect_with_deps(
            move |_| {
                let interval = Interval::new(5000, move || {
                    if let Err(e) = state.save() {
                        console::log_1(&format!("Save error: {}", e).into());
                    }
                });
                move || drop(interval)
            },
            *interval_key,
        );
    }

    // Setup tick interval
    {
        let state = state.clone();
        use_effect_with_deps(
            move |_| {
                let interval = Interval::new(1000, move || {
                    console::log_1(&"tick".into());
                    state.dispatch(Msg::Tick);
                });
                move || drop(interval)
            },
            *interval_key,
        );
    }

    // Update interval_key when loading
    let on_load = {
        let interval_key = interval_key.clone();
        let state = state.clone();
        Callback::from(move |_| {
            state.dispatch(Msg::Load);
            interval_key.set(*interval_key + 1); // Force interval recreation
        })
    };

    html! {
        <div class="p-4 max-w-2xl mx-auto">
            <h1 class="text-3xl font-bold mb-4 text-center">{ "Idle Game with Big Numbers" }</h1>
            
            <div class="bg-gray-100 rounded-lg p-4 mb-4">
                <div class="grid grid-cols-2 gap-4">
                    <div class="bg-white p-3 rounded shadow">
                        <div class="text-gray-600 text-sm">{ "Counter" }</div>
                        <div class="text-2xl font-bold">{ format_number(&(*state).counter) }</div>
                    </div>
                    <div class="bg-white p-3 rounded shadow">
                        <div class="text-gray-600 text-sm">{ "Production per second" }</div>
                        <div class="text-2xl font-bold">{ format_number(&(*state).production) }</div>
                    </div>
                </div>
            </div>

            <div class="flex flex-col gap-2">
                <button 
                    class="px-4 py-3 bg-blue-500 text-white rounded hover:bg-blue-600 transition-colors"
                    onclick={create_dispatch_callback(state.clone(), Msg::UpgradeProduction)}>
                    { "Upgrade Production (Double)" }
                </button>
                
                <div class="flex gap-2">
                    <button 
                        class="flex-1 px-4 py-2 bg-green-500 text-white rounded hover:bg-green-600 transition-colors"
                        onclick={create_dispatch_callback(state.clone(), Msg::Save)}>
                        { "Save Game" }
                    </button>
                    <button 
                        class="flex-1 px-4 py-2 bg-yellow-500 text-white rounded hover:bg-yellow-600 transition-colors"
                        onclick={on_load}>
                        { "Load Game" }
                    </button>
                    <button 
                        class="flex-1 px-4 py-2 bg-red-500 text-white rounded hover:bg-red-600 transition-colors"
                        onclick={create_dispatch_callback(state.clone(), Msg::Reset)}>
                        { "Reset Game" }
                    </button>
                </div>
            </div>

            <div class="mt-4 text-sm text-gray-500 text-center">
                { format!("Last saved: {}", state.format_last_saved()) }
            </div>
        </div>
    }
}

fn create_dispatch_callback(state: UseReducerHandle<State>, msg: Msg) -> Callback<MouseEvent> {
    Callback::from(move |_| state.dispatch(msg.clone()))
}
