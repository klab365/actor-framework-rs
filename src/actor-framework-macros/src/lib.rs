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

/// Adapt a reusable async handler to typed actor route capabilities.
///
/// Context parameters use `ActorAddress<ActorCommand<Message>>` or
/// `ActorAddress<ActorEvent<Event>>`. The generated helper binds the concrete
/// system router at the composition site.
#[proc_macro_attribute]
pub fn actor_handler(_attribute: TokenStream, input: TokenStream) -> TokenStream {
    let mut function = parse_macro_input!(input as syn::ItemFn);
    if function.sig.asyncness.is_none() {
        return syn::Error::new_spanned(&function.sig, "`actor_handler` requires an async function")
            .into_compile_error()
            .into();
    }

    let router = format_ident!("__ActorFrameworkRouter");
    let mut contexts = Vec::new();
    for (index, argument) in function.sig.inputs.iter_mut().enumerate() {
        let syn::FnArg::Typed(argument) = argument else {
            return syn::Error::new_spanned(argument, "actor handlers must be free functions")
                .into_compile_error()
                .into();
        };
        let syn::Type::Path(path) = argument.ty.as_ref() else {
            continue;
        };
        let Some(segment) = path.path.segments.last() else {
            continue;
        };
        if segment.ident != "ActorAddress" {
            continue;
        }
        let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
            return syn::Error::new_spanned(
                &argument.ty,
                "expected `ActorAddress<ActorCommand<M>>` or `ActorAddress<ActorEvent<E>>`",
            )
            .into_compile_error()
            .into();
        };
        let Some(syn::GenericArgument::Type(marker_ty)) = args.args.first() else {
            return syn::Error::new_spanned(&argument.ty, "expected a command/event marker type")
                .into_compile_error()
                .into();
        };
        let syn::Type::Path(marker_path) = marker_ty else {
            return syn::Error::new_spanned(marker_ty, "expected ActorCommand<M> or ActorEvent<E>")
                .into_compile_error()
                .into();
        };
        let Some(marker_segment) = marker_path.path.segments.last() else {
            unreachable!()
        };
        let kind = marker_segment.ident.to_string();
        if kind != "ActorCommand" && kind != "ActorEvent" {
            return syn::Error::new_spanned(marker_ty, "expected ActorCommand<M> or ActorEvent<E>")
                .into_compile_error()
                .into();
        }
        let syn::PathArguments::AngleBracketed(marker_args) = &marker_segment.arguments else {
            return syn::Error::new_spanned(marker_ty, "message marker must have a message type")
                .into_compile_error()
                .into();
        };
        let Some(syn::GenericArgument::Type(message)) = marker_args.args.first() else {
            return syn::Error::new_spanned(marker_ty, "message marker must have a message type")
                .into_compile_error()
                .into();
        };
        let message = message.clone();
        let marker_ident = &marker_segment.ident;
        let marker = quote!(::actor_framework::actor_address::#marker_ident<#message>);
        *argument.ty = syn::parse_quote!(::actor_framework::actor_address::ActorAddress<'_, #marker, #router>);
        contexts.push((index, kind, message, marker));
    }
    if !contexts.is_empty() {
        function.sig.generics.params.push(syn::parse_quote!(#router));
        for (_, kind, message, _) in &contexts {
            let predicate = if kind == "ActorCommand" {
                quote!(#router: ::actor_framework::ActorSendRoute<#message>)
            } else {
                quote!(#router: ::actor_framework::ActorPublishRoute<#message>)
            };
            function
                .sig
                .generics
                .make_where_clause()
                .predicates
                .push(syn::parse_quote!(#predicate));
        }
    }

    let name = function.sig.ident.clone();
    let helper = format_ident!("__actor_handler_{}", name);
    let visibility = function.vis.clone();
    let mut helper_args = Vec::new();
    let call_args: Vec<_> = function
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(index, arg)| {
            if contexts.iter().any(|(context_index, ..)| *context_index == index) {
                quote!(::actor_framework::actor_address::ActorAddress::new(router))
            } else if let syn::FnArg::Typed(typed) = arg {
                let ident = format_ident!("__actor_handler_arg_{index}");
                let ty = &typed.ty;
                helper_args.push(quote!(#ident: #ty));
                quote!(#ident)
            } else {
                unreachable!()
            }
        })
        .collect();
    let bounds = contexts.iter().map(|(_, kind, message, _)| {
        if kind == "ActorCommand" {
            quote!(#router: ::actor_framework::ActorSendRoute<#message>,)
        } else {
            quote!(#router: ::actor_framework::ActorPublishRoute<#message>,)
        }
    });
    let has_contexts = !contexts.is_empty();
    let helper_router_param = quote!(router: &#router,);
    let helper_generics = quote!(<#router>);
    let helper_where = has_contexts.then(|| quote!(where #(#bounds)*));
    let output = &function.sig.output;
    quote! {
        #function
        #[doc(hidden)]
        #visibility async fn #helper #helper_generics (
            #helper_router_param
            #(#helper_args),*
        ) #output
        #helper_where
        {
            #name(#(#call_args),*).await
        }
    }
    .into()
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
    let actor_tokens = actors.iter().map(quote_actor);
    let event_tokens = events.iter().map(module_type);
    quote! {
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
}

impl Parse for ModuleInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let _visibility: syn::Visibility = input.parse()?;
        expect_ident(input, "manifest")?;
        let manifest = input.parse()?;
        input.parse::<Token![;]>()?;
        let mut events = Vec::new();
        if input.peek(Ident) {
            let fork = input.fork();
            let section: Ident = fork.parse()?;
            if section == "events" {
                expect_ident(input, "events")?;
                input.parse::<Token![:]>()?;
                let event_body;
                syn::bracketed!(event_body in input);
                events = Punctuated::<Type, Token![,]>::parse_terminated(&event_body)?
                    .into_iter()
                    .collect();
                input.parse::<Token![;]>()?;
            }
        }
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

fn handler_adapter(path: &Path) -> proc_macro2::TokenStream {
    let mut adapter = path.clone();
    let last = adapter.segments.last_mut().expect("handler path is nonempty");
    last.ident = format_ident!("__actor_handler_{}", last.ident);
    last.arguments = syn::PathArguments::None;
    quote!(#adapter)
}

fn module_handler(path: &Path) -> proc_macro2::TokenStream {
    if path.leading_colon.is_none() && path.segments.len() == 1 {
        quote!($crate::#path)
    } else {
        quote!(#path)
    }
}

fn quote_actor(actor: &ActorDef) -> proc_macro2::TokenStream {
    let field = &actor.field;
    let state = module_type(&actor.state);
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
        let arms = actor
            .commands
            .iter()
            .chain(actor.events.iter())
            .enumerate()
            .map(|(index, route)| {
                let variant = format_ident!("Message{}", index);
                let handler = handler_adapter(&route.handler);
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
                impl ::actor_framework::ActorTrySendRoute<#message> for #context_type {
                    fn try_send_route(&self, message: #message) -> Result<(), ::actor_framework::TrySendError<#message>> {
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

    Ok(quote! {
        #(#enum_defs)*

        #[doc(hidden)]
        #[doc = "Application-wide typed actor messaging context generated by `actor_system!`."]
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
            where #context_type: ::actor_framework::ActorTrySendRoute<M> {
                <#context_type as ::actor_framework::ActorTrySendRoute<M>>::try_send_route(&self.__context, message)
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
