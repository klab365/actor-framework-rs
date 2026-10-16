mod embassy;
mod wasm;

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, Ident, Path, Result, Token, Type,
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
};

#[derive(Clone)]
struct SystemInput {
    visibility: syn::Visibility,
    name: Ident,
    actors: Vec<ActorDef>,
    events: Vec<Type>,
}

#[derive(Clone)]
struct ActorDef {
    field: Ident,
    state: Type,
    capacity: Expr,
    context: Option<Type>,
    commands: Vec<RouteDef>,
    events: Vec<RouteDef>,
}

#[derive(Clone)]
struct RouteDef {
    message: Type,
    handler: Path,
}

impl Parse for SystemInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let visibility: syn::Visibility = input.parse()?;
        input.parse::<Token![struct]>()?;
        let name = input.parse()?;
        let body;
        syn::braced!(body in input);
        expect_ident(&body, "actors")?;
        body.parse::<Token![:]>()?;
        let actors_body;
        syn::bracketed!(actors_body in body);
        let actors = Punctuated::<ActorDef, Token![,]>::parse_terminated(&actors_body)?
            .into_iter()
            .collect();
        body.parse::<Token![;]>()?;
        let mut events = Vec::new();
        if !body.is_empty() {
            expect_ident(&body, "events")?;
            body.parse::<Token![:]>()?;
            let event_body;
            syn::bracketed!(event_body in body);
            events = Punctuated::<Type, Token![,]>::parse_terminated(&event_body)?
                .into_iter()
                .collect();
            body.parse::<Token![;]>()?;
        }
        Ok(Self {
            visibility,
            name,
            actors,
            events,
        })
    }
}

impl Parse for ActorDef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let field = input.parse()?;
        input.parse::<Token![:]>()?;
        let state = input.parse()?;
        let body;
        syn::braced!(body in input);
        let mut context = None;
        if body.peek(Ident) {
            let fork = body.fork();
            let key: Ident = fork.parse()?;
            if key == "context" {
                expect_ident(&body, "context")?;
                body.parse::<Token![:]>()?;
                context = Some(body.parse()?);
                body.parse::<Token![;]>()?;
            }
        }
        expect_ident(&body, "mailbox_capacity")?;
        body.parse::<Token![:]>()?;
        let capacity = body.parse()?;
        body.parse::<Token![;]>()?;
        let mut commands = Vec::new();
        let mut events = Vec::new();
        while !body.is_empty() {
            let section: Ident = body.parse()?;
            let route_body;
            syn::braced!(route_body in body);
            let routes = Punctuated::<RouteDef, Token![,]>::parse_terminated(&route_body)?;
            match section.to_string().as_str() {
                "commands" => commands.extend(routes),
                "events" => events.extend(routes),
                _ => {
                    return Err(syn::Error::new(section.span(), "expected `commands` or `events`"));
                }
            }
        }
        Ok(Self {
            field,
            state,
            capacity,
            context,
            commands,
            events,
        })
    }
}

impl Parse for RouteDef {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let message = input.parse()?;
        input.parse::<Token![=>]>()?;
        let handler = input.parse()?;
        if input.peek(Token![;]) {
            input.parse::<Token![;]>()?;
        }
        Ok(Self { message, handler })
    }
}

/// Internal endpoint used after declarative module composition.
#[doc(hidden)]
#[proc_macro]
pub fn __actor_system(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SystemInput);
    match expand(input, false) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Internal WASM endpoint after module composition.
#[doc(hidden)]
#[proc_macro]
pub fn __actor_system_wasm(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SystemInput);
    match expand(input, true) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

/// Internal Embassy endpoint after actor-module composition.
#[doc(hidden)]
#[proc_macro]
pub fn __actor_system_embassy(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as SystemInput);
    match embassy::expand(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.into_compile_error().into(),
    }
}

fn expect_ident(input: ParseStream<'_>, expected: &str) -> Result<()> {
    let actual: Ident = input.parse()?;
    if actual == expected {
        Ok(())
    } else {
        Err(syn::Error::new(actual.span(), format!("expected `{expected}`")))
    }
}

/// Create an actor manifest macro that can be consumed by `actor_system!`.
#[proc_macro]
pub fn actor_module(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as ModuleInput);
    let manifest = input.manifest;
    let actors = input.actors;
    let events = input.events;
    let sends = input.sends;
    let publishes = input.publishes;
    let actor_tokens = actors.iter().map(|actor| quote_actor(actor, &manifest));
    let event_tokens = events.iter().map(module_type);
    let send_bounds = sends
        .iter()
        .map(|message| quote! { + ::actor_framework::ActorSendRoute<#message> });
    let publish_bounds = publishes
        .iter()
        .map(|message| quote! { + ::actor_framework::ActorPublishRoute<#message> });
    quote! {
        pub mod #manifest {
            use super::*;
            pub trait ActorContext: ::actor_framework::ActorContext #(#send_bounds)* #(#publish_bounds)* {}
        }
        #[macro_export]
        macro_rules! #manifest {
            ($callback:path, { $vis:vis struct $name:ident; actors: [$($existing:tt)*]; events: [$($known_events:ty,)*]; remaining: [$($remaining:ident,)*]; }) => {
                $callback! {
                    $vis struct $name;
                    actors: [$($existing)* #(#actor_tokens,)*];
                    events: [$($known_events,)* #(#event_tokens,)*];
                    remaining: [$($remaining,)*];
                }
            };
        }
    }
    .into()
}

struct ModuleInput {
    manifest: Ident,
    actors: Vec<ActorDef>,
    events: Vec<Type>,
    sends: Vec<Type>,
    publishes: Vec<Type>,
}

impl Parse for ModuleInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let _visibility: syn::Visibility = input.parse()?;
        expect_ident(input, "manifest")?;
        let manifest = input.parse()?;
        input.parse::<Token![;]>()?;
        expect_ident(input, "context")?;
        let context_body;
        syn::braced!(context_body in input);
        let mut sends = Vec::new();
        let mut publishes = Vec::new();
        while !context_body.is_empty() {
            let capability: Ident = context_body.parse()?;
            context_body.parse::<Token![:]>()?;
            let types_body;
            syn::bracketed!(types_body in context_body);
            let types = Punctuated::<Type, Token![,]>::parse_terminated(&types_body)?
                .into_iter()
                .collect::<Vec<_>>();
            context_body.parse::<Token![;]>()?;
            match capability.to_string().as_str() {
                "send" => sends.extend(types),
                "publish" => publishes.extend(types),
                _ => return Err(syn::Error::new(capability.span(), "expected `send` or `publish`")),
            }
        }
        expect_ident(input, "events")?;
        input.parse::<Token![:]>()?;
        let event_body;
        syn::bracketed!(event_body in input);
        let events = Punctuated::<Type, Token![,]>::parse_terminated(&event_body)?
            .into_iter()
            .collect();
        input.parse::<Token![;]>()?;
        expect_ident(input, "actors")?;
        input.parse::<Token![:]>()?;
        let actors_body;
        syn::bracketed!(actors_body in input);
        let actors = Punctuated::<ActorDef, Token![,]>::parse_terminated(&actors_body)?
            .into_iter()
            .collect();
        input.parse::<Token![;]>()?;
        Ok(Self {
            manifest,
            actors,
            events,
            sends,
            publishes,
        })
    }
}

fn module_type(ty: &Type) -> proc_macro2::TokenStream {
    const PRELUDE_TYPES: &[&str] = &[
        "bool", "char", "str", "u8", "u16", "u32", "u64", "u128", "usize", "i8", "i16", "i32", "i64", "i128", "isize",
        "f32", "f64", "String", "Vec", "Option", "Result", "Box", "Self",
    ];
    match ty {
        Type::Path(path)
            if path.qself.is_none()
                && path.path.leading_colon.is_none()
                && path.path.segments.len() == 1
                && !PRELUDE_TYPES.contains(&path.path.segments[0].ident.to_string().as_str()) =>
        {
            quote!($crate::#ty)
        }
        _ => quote!(#ty),
    }
}

fn module_handler(path: &Path) -> proc_macro2::TokenStream {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        quote!($crate::#path)
    } else {
        quote!(#path)
    }
}

fn quote_actor(actor: &ActorDef, module: &Ident) -> proc_macro2::TokenStream {
    let field = &actor.field;
    let state = module_type(&actor.state);
    let context = quote!($crate::#module::ActorContext);
    let capacity = &actor.capacity;
    let commands = actor.commands.iter().map(|route| {
        let message = module_type(&route.message);
        let handler = module_handler(&route.handler);
        quote! { #message => #handler; }
    });
    let events = actor.events.iter().map(|route| {
        let message = module_type(&route.message);
        let handler = module_handler(&route.handler);
        quote! { #message => #handler; }
    });
    quote! {
        #field: #state {
            context: #context;
            mailbox_capacity: #capacity;
            commands { #(#commands)* }
            events { #(#events)* }
        }
    }
}

fn expand(input: SystemInput, wasm: bool) -> Result<proc_macro2::TokenStream> {
    if input.actors.is_empty() {
        return Err(syn::Error::new(
            input.name.span(),
            "an actor system needs at least one actor",
        ));
    }
    let SystemInput {
        visibility,
        name,
        actors,
        events: declared_events,
    } = input;
    let mut seen_commands = Vec::<Type>::new();
    for actor in &actors {
        for route in &actor.commands {
            let route_ty = &route.message;
            if seen_commands
                .iter()
                .any(|ty| quote!(#ty).to_string() == quote!(#route_ty).to_string())
            {
                return Err(syn::Error::new_spanned(
                    &route.message,
                    "a command may have only one actor handler",
                ));
            }
            seen_commands.push(route.message.clone());
        }
    }

    if wasm {
        return Ok(wasm::expand(visibility, name, actors, declared_events));
    }

    let enum_defs = actors.iter().map(|actor| {
        let msg_enum = format_ident!("__{}MailboxMessage", actor.field);
        let variants = actor
            .commands
            .iter()
            .chain(actor.events.iter())
            .enumerate()
            .map(|(index, route)| {
                let variant = format_ident!("Message{}", index);
                let ty = &route.message;
                quote! { #variant(#ty) }
            });
        quote! { #[allow(non_camel_case_types)] enum #msg_enum { #(#variants),* } }
    });
    let context_type = format_ident!("__{}Context", name);
    let handle_fields: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            let msg_enum = format_ident!("__{}MailboxMessage", field);
            quote! { #field: ::tokio::sync::mpsc::Sender<#msg_enum> }
        })
        .collect();
    let channel_creation = actors.iter().map(|actor| {
        let field = &actor.field;
        let receiver = format_ident!("__{}_receiver", field);
        let msg_enum = format_ident!("__{}MailboxMessage", field);
        let capacity = &actor.capacity;
        quote! {
            let (#field, mut #receiver) = ::tokio::sync::mpsc::channel::<#msg_enum>(#capacity);
        }
    });
    let clone_fields: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            quote! { #field: self.#field.clone() }
        })
        .collect();
    let context_fields: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            quote! { #field: #field.clone() }
        })
        .collect();
    let builder_name = format_ident!("{}Builder", name);
    let builder_fields: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            let state = &actor.state;
            quote! { #field: Option<#state> }
        })
        .collect();
    let builder_empty: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            quote! { #field: None }
        })
        .collect();
    let builder_setters: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            let state = &actor.state;
            quote! { pub fn #field(mut self, state: #state) -> Self { self.#field = Some(state); self } }
        })
        .collect();
    let state_extracts: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            let state = format_ident!("__{}_state", field);
            quote! { let mut #state = self.#field.ok_or(::actor_framework::MissingActorState(stringify!(#field)))?; }
        })
        .collect();
    let task_done_fields: Vec<_> = actors
        .iter()
        .map(|actor| {
            let receiver = format_ident!("__{}_done_rx", actor.field);
            let field = &actor.field;
            quote! { (stringify!(#field), #receiver) }
        })
        .collect();
    let completion_channels: Vec<_> = actors
        .iter()
        .map(|actor| {
            let field = &actor.field;
            let done_tx = format_ident!("__{}_done_tx", field);
            let done_rx = format_ident!("__{}_done_rx", field);
            quote! { let (#done_tx, #done_rx) = ::tokio::sync::watch::channel(false); }
        })
        .collect();
    let task_spawns = actors.iter().map(|actor| {
        let field = &actor.field;
        let receiver = format_ident!("__{}_receiver", field);
        let state = format_ident!("__{}_state", field);
        let done_tx = format_ident!("__{}_done_tx", field);
        let msg_enum = format_ident!("__{}MailboxMessage", field);
        let arms = actor.commands.iter().chain(actor.events.iter()).enumerate().map(|(index, route)| {
            let variant = format_ident!("Message{}", index);
            let handler = &route.handler;
            quote! { #msg_enum::#variant(message) => { let () = #handler(&task_context, &mut #state, message).await; } }
        });
        quote! {
            let task_context = system.__context.clone();
            let mut shutdown = task_context.__shutdown.clone();
            ::tokio::spawn(async move {
                loop {
                    ::tokio::select! {
                        biased;
                        _ = shutdown.changed() => break,
                        message = #receiver.recv() => match message {
                            Some(message) => match message { #(#arms),* },
                            None => break,
                        },
                    }
                }
                #done_tx.send_replace(true);
            });
        }
    });

    let mut route_impls = Vec::new();
    let mut try_route_impls = Vec::new();
    for actor in &actors {
        let field = &actor.field;
        let msg_enum = format_ident!("__{}MailboxMessage", field);
        for (index, route) in actor.commands.iter().enumerate() {
            let message = &route.message;
            let variant = format_ident!("Message{}", index);
            route_impls.push(quote! {
                impl ::actor_framework::ActorSendRoute<#message> for #context_type {
                    async fn send_route(&self, message: #message) -> Result<(), ::actor_framework::SendError> {
                        if *self.__shutdown.borrow() { return Err(::actor_framework::SendError); }
                        let mut shutdown = self.__shutdown.clone();
                        ::tokio::select! {
                            biased;
                            _ = shutdown.changed() => Err(::actor_framework::SendError),
                            result = self.#field.send(#msg_enum::#variant(message)) => result.map_err(|_| ::actor_framework::SendError),
                        }
                    }
                }
            });
            try_route_impls.push(quote! {
                impl __ActorTrySendRoute<#message> for #context_type {
                    fn try_route(&self, message: #message) -> Result<(), ::actor_framework::TrySendError<#message>> {
                        if *self.__shutdown.borrow() { return Err(::actor_framework::TrySendError::Closed(message)); }
                        match self.#field.try_reserve() {
                            Ok(permit) => { permit.send(#msg_enum::#variant(message)); Ok(()) }
                            Err(::tokio::sync::mpsc::error::TrySendError::Full(())) => Err(::actor_framework::TrySendError::Full(message)),
                            Err(::tokio::sync::mpsc::error::TrySendError::Closed(())) => Err(::actor_framework::TrySendError::Closed(message)),
                        }
                    }
                }
            });
        }
    }

    let mut event_route_groups: Vec<(Type, Vec<proc_macro2::TokenStream>)> =
        declared_events.into_iter().map(|event| (event, Vec::new())).collect();
    for actor in &actors {
        let field = &actor.field;
        let msg_enum = format_ident!("__{}MailboxMessage", field);
        let offset = actor.commands.len();
        for (index, route) in actor.events.iter().enumerate() {
            let message = &route.message;
            if !event_route_groups
                .iter()
                .any(|(ty, _)| quote!(#ty).to_string() == quote!(#message).to_string())
            {
                event_route_groups.push((message.clone(), Vec::new()));
            }
            let variant = format_ident!("Message{}", offset + index);
            let send = quote! {
                if *self.__shutdown.borrow() { return Err(::actor_framework::SendError); }
                let mut shutdown = self.__shutdown.clone();
                ::tokio::select! {
                    biased;
                    _ = shutdown.changed() => return Err(::actor_framework::SendError),
                    result = self.#field.send(#msg_enum::#variant(message.clone())) => result.map_err(|_| ::actor_framework::SendError)?,
                }
            };
            if let Some((_, sends)) = event_route_groups
                .iter_mut()
                .find(|(ty, _)| quote!(#ty).to_string() == quote!(#message).to_string())
            {
                sends.push(send);
            } else {
                event_route_groups.push((message.clone(), vec![send]));
            }
        }
    }
    for (message, sends) in event_route_groups {
        route_impls.push(quote! {
            impl ::actor_framework::ActorPublishRoute<#message> for #context_type {
                async fn publish_route(&self, message: #message) -> Result<(), ::actor_framework::SendError> {
                    #(#sends)*
                    Ok(())
                }
            }
        });
    }

    let mut seen_contexts = Vec::<String>::new();
    let context_impls: Vec<_> = actors
        .iter()
        .filter_map(|actor| {
            let context = actor.context.as_ref()?;
            let key = quote!(#context).to_string();
            if seen_contexts.contains(&key) {
                return None;
            }
            seen_contexts.push(key);
            Some(quote! { impl #context for #context_type {} })
        })
        .collect();

    Ok(quote! {
        #(#enum_defs)*

        #[doc(hidden)]
        pub trait __ActorTrySendRoute<M> {
            fn try_route(&self, message: M) -> Result<(), ::actor_framework::TrySendError<M>>;
        }

        #[doc(hidden)]
        struct #context_type {
            #(#handle_fields),*,
            __shutdown: ::tokio::sync::watch::Receiver<bool>,
        }

        impl Clone for #context_type {
            fn clone(&self) -> Self {
                Self { #(#clone_fields),*, __shutdown: self.__shutdown.clone() }
            }
        }

        #visibility struct #name {
            __context: #context_type,
            __shutdown: ::tokio::sync::watch::Sender<bool>,
            __tasks_done: ::tokio::sync::Mutex<Vec<(&'static str, ::tokio::sync::watch::Receiver<bool>)>>,
        }

        #visibility struct #builder_name {
            #(#builder_fields),*
        }

        #(#route_impls)*
        #(#try_route_impls)*
        #(#context_impls)*

        impl Drop for #name {
            fn drop(&mut self) {
                self.__shutdown.send_replace(true);
            }
        }

        impl #name {
            pub fn builder() -> #builder_name {
                #builder_name { #(#builder_empty),* }
            }

            pub async fn send<M>(&self, message: M) -> Result<(), ::actor_framework::SendError>
            where #context_type: ::actor_framework::ActorSendRoute<M> {
                <#context_type as ::actor_framework::ActorSendRoute<M>>::send_route(&self.__context, message).await
            }

            pub fn try_send<M>(&self, message: M) -> Result<(), ::actor_framework::TrySendError<M>>
            where #context_type: __ActorTrySendRoute<M> {
                <#context_type as __ActorTrySendRoute<M>>::try_route(&self.__context, message)
            }

            pub async fn publish<M>(&self, message: M) -> Result<(), ::actor_framework::SendError>
            where #context_type: ::actor_framework::ActorPublishRoute<M> {
                <#context_type as ::actor_framework::ActorPublishRoute<M>>::publish_route(&self.__context, message).await
            }

            /// Signal actors to stop after their current message and wait for every task to exit.
            pub async fn shutdown(&self) -> Result<(), ::actor_framework::ActorTaskError> {
                self.__shutdown.send_replace(true);
                let receivers = self.__tasks_done.lock().await.clone();
                let mut first_error = None;
                for (actor, mut receiver) in receivers {
                    if !*receiver.borrow_and_update() && receiver.changed().await.is_err() && first_error.is_none() {
                        first_error = Some(::actor_framework::ActorTaskError { actor });
                    }
                }
                first_error.map_or(Ok(()), Err)
            }
        }

        impl #builder_name {
            #(#builder_setters)*

            pub fn start(self) -> Result<#name, ::actor_framework::MissingActorState> {
                #(#state_extracts)*
                #(#channel_creation)*
                let (__shutdown, shutdown_rx) = ::tokio::sync::watch::channel(false);
                #(#completion_channels)*
                let context = #context_type {
                    #(#context_fields),*,
                    __shutdown: shutdown_rx,
                };
                let system = #name {
                    __context: context,
                    __shutdown,
                    __tasks_done: ::tokio::sync::Mutex::new(vec![#(#task_done_fields),*]),
                };
                #(#task_spawns)*
                Ok(system)
            }
        }
    })
}
