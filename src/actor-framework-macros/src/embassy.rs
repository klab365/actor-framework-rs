//! Embassy-specific `actor_system!` expansion.
use crate::SystemInput;
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::Result;

fn actor_handler_adapter(handler: &syn::Path) -> TokenStream {
    let mut adapter = handler.clone();
    let last = adapter
        .segments
        .last_mut()
        .expect("a Rust path contains at least one segment");
    last.ident = format_ident!("__actor_handler_{}", last.ident);
    last.arguments = syn::PathArguments::None;
    quote!(#adapter)
}

pub(super) fn expand(input: SystemInput) -> Result<TokenStream> {
    if input.actors.is_empty() {
        return Err(syn::Error::new(
            input.name.span(),
            "an actor system needs at least one actor",
        ));
    }
    let mut commands = Vec::<syn::Type>::new();
    for actor in &input.actors {
        for route in &actor.commands {
            let message = &route.message;
            if commands
                .iter()
                .any(|previous| quote!(#previous).to_string() == quote!(#message).to_string())
            {
                return Err(syn::Error::new_spanned(
                    message,
                    "a command may have only one actor handler",
                ));
            }
            commands.push(message.clone());
        }
    }
    let name = input.name;
    let visibility = input.visibility;
    let builder = format_ident!("{}Builder", name);
    let context = format_ident!("__{}EmbassyContext", name);
    let worker_count = input.actors.len();
    let mut enums = Vec::new();
    let mut context_fields = Vec::new();
    let mut builder_fields = Vec::new();
    let mut empty = Vec::new();
    let mut setters = Vec::new();
    let mut extracts = Vec::new();
    let mut validate_capacities = Vec::new();
    let mut statics = Vec::new();
    let mut channel_init = Vec::new();
    let mut task_fns = Vec::new();
    let mut token_preparation = Vec::new();
    let mut token_spawns = Vec::new();
    let mut command_routes = Vec::new();
    let mut shutdown_sends = Vec::new();
    let mut event_groups: Vec<(syn::Type, Vec<TokenStream>)> =
        input.events.into_iter().map(|t| (t, Vec::new())).collect();

    for actor in input.actors.clone() {
        let field = actor.field;
        let state = actor.state;
        let capacity = actor.capacity;
        let msg = format_ident!("__{}{}Message", name, field);
        let task = format_ident!("__{}_{}_task", name.to_string().to_lowercase(), field);
        let rx = format_ident!("__{}{}_receiver", name, field);
        let channel = format_ident!("__{}{}_channel", name, field);
        let cell = format_ident!("__{}{}_cell", name, field);
        let state_var = format_ident!("__{}{}_state", name, field);
        let sender_ty = quote!(::actor_framework::__embassy_sync::channel::Sender<'static, ::actor_framework::__embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, #msg, #capacity>);
        let routes: Vec<_> = actor.commands.iter().chain(actor.events.iter()).collect();
        let variants: Vec<_> = routes
            .iter()
            .enumerate()
            .map(|(i, r)| {
                let v = format_ident!("Message{i}");
                let ty = &r.message;
                quote!(#v(#ty))
            })
            .collect();
        enums.push(quote!(#[allow(non_camel_case_types)] enum #msg { #(#variants,)* Shutdown }));
        context_fields.push(quote!(#field: #sender_ty));
        builder_fields.push(quote!(#field: Option<#state>));
        empty.push(quote!(#field: None));
        setters.push(quote!(pub fn #field(mut self, state: #state) -> Self { self.#field = Some(state); self }));
        extracts.push(quote!(let mut #state_var = self.#field.ok_or(::actor_framework::embassy::StartError::MissingActorState(stringify!(#field)))?;));
        validate_capacities.push(quote!(if (#capacity) == 0 { return Err(::actor_framework::embassy::StartError::InvalidCapacity(stringify!(#field))); }));
        let ty = quote!(::actor_framework::__embassy_sync::channel::Channel<::actor_framework::__embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, #msg, #capacity>);
        statics.push(quote!(static #cell: #ty = ::actor_framework::__embassy_sync::channel::Channel::new();));
        shutdown_sends.push(quote!(context.#field.send(#msg::Shutdown).await;));
        channel_init.push(quote! {
            let #channel: &'static #ty = &#cell;
            let #field = #channel.sender();
            let #rx = #channel.receiver();
        });
        let dispatch: Vec<_> = routes
            .iter()
            .enumerate()
            .map(|(i, route)| {
                let v = format_ident!("Message{i}");
                let handler = actor_handler_adapter(&route.handler);
                quote!(#msg::#v(message) => { let () = #handler(&task_context, &mut actor, message).await; })
            })
            .collect();
        task_fns.push(quote! {
            #[allow(non_snake_case)]
            #[embassy_executor::task(pool_size = 1)]
            async fn #task(mut actor: #state, mut receiver: ::actor_framework::__embassy_sync::channel::Receiver<'static, ::actor_framework::__embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex, #msg, #capacity>, task_context: #context, lifecycle: &'static ::actor_framework::embassy::Lifecycle) {
                loop { match receiver.receive().await { #msg::Shutdown => break, message => match message { #(#dispatch),* #msg::Shutdown => break } } }
                lifecycle.complete();
            }
        });
        let token = format_ident!("__{}_{}_token", name.to_string().to_lowercase(), field);
        token_preparation.push(quote! {
            let #token = match #task(#state_var, #rx, task_context.clone(), lifecycle) {
                Ok(token) => token,
                Err(_) => { lifecycle.abort_start(); return Err(::actor_framework::embassy::StartError::TaskCapacityExhausted); }
            };
        });
        token_spawns.push(quote!(spawner.spawn(#token);));
        for (i, route) in actor.commands.iter().enumerate() {
            let message = &route.message;
            let variant = format_ident!("Message{i}");
            command_routes.push(quote! {
                impl ::actor_framework::ActorSendRoute<#message> for #context {
                    async fn send_route(&self, message: #message) -> Result<(), ::actor_framework::SendError> {
                        if self.__lifecycle.is_closed() { return Err(::actor_framework::SendError); }
                        match ::actor_framework::__embassy_futures::select::select(
                            self.__lifecycle.wait_closed(),
                            self.#field.send(#msg::#variant(message)),
                        ).await {
                            ::actor_framework::__embassy_futures::select::Either::First(()) => Err(::actor_framework::SendError),
                            ::actor_framework::__embassy_futures::select::Either::Second(()) => Ok(()),
                        }
                    }
                }
                impl ::actor_framework::ActorTrySendRoute<#message> for #context {
                    fn try_send_route(&self, message: #message) -> Result<(), ::actor_framework::TrySendError<#message>> {
                        if self.__lifecycle.is_closed() { return Err(::actor_framework::TrySendError::Closed(message)); }
                        match self.#field.try_send(#msg::#variant(message)) { Ok(()) => Ok(()), Err(::actor_framework::__embassy_sync::channel::TrySendError::Full(#msg::#variant(m))) => Err(::actor_framework::TrySendError::Full(m)), _ => unreachable!() }
                    }
                }
            });
        }
        let offset = actor.commands.len();
        for (i, route) in actor.events.iter().enumerate() {
            let message = &route.message;
            let variant = format_ident!("Message{}", offset + i);
            let sends = event_groups
                .iter_mut()
                .find(|(ty, _)| quote!(#ty).to_string() == quote!(#message).to_string());
            let entry = quote! {
                if self.__lifecycle.is_closed() { return Err(::actor_framework::SendError); }
                match ::actor_framework::__embassy_futures::select::select(
                    self.__lifecycle.wait_closed(),
                    self.#field.send(#msg::#variant(message.clone())),
                ).await {
                    ::actor_framework::__embassy_futures::select::Either::First(()) => return Err(::actor_framework::SendError),
                    ::actor_framework::__embassy_futures::select::Either::Second(()) => {},
                }
            };
            if let Some((_, sends)) = sends {
                sends.push(entry);
            } else {
                event_groups.push((message.clone(), vec![entry]));
            }
        }
    }
    let shutdown_task = format_ident!("__{}_shutdown_task", name.to_string().to_lowercase());
    let shutdown_token = format_ident!("__{}_shutdown_token", name.to_string().to_lowercase());
    let lifecycle_cell = format_ident!("__{}Lifecycle", name);
    task_fns.push(quote! {
        #[embassy_executor::task(pool_size = 1)]
        async fn #shutdown_task(context: #context, lifecycle: &'static ::actor_framework::embassy::Lifecycle) {
            lifecycle.shutdown_requested().await;
            #(#shutdown_sends)*
        }
    });
    token_preparation.push(quote! {
        let #shutdown_token = match #shutdown_task(task_context.clone(), lifecycle) {
            Ok(token) => token,
            Err(_) => { lifecycle.abort_start(); return Err(::actor_framework::embassy::StartError::TaskCapacityExhausted); }
        };
    });
    token_spawns.push(quote!(spawner.spawn(#shutdown_token);));

    let publish = event_groups.iter().map(|(ty,sends)| quote! {
        impl ::actor_framework::ActorPublishRoute<#ty> for #context {
            async fn publish_route(&self, message: #ty) -> Result<(), ::actor_framework::SendError> { if self.__lifecycle.is_closed() { return Err(::actor_framework::SendError); } #(#sends)* Ok(()) }
        }
    });
    let handle_fields = input.actors.iter().map(|a| {
        let f = &a.field;
        quote!(#f: task_context.#f)
    });
    let fields = input.actors.iter().map(|a| {
        let f = &a.field;
        quote!(#f: #f)
    });
    let handle_sends = quote! {};
    Ok(quote! {
        #(#enums)*
        #(#task_fns)*
        #[derive(Clone)] struct #context { #(#context_fields),*, __lifecycle: &'static ::actor_framework::embassy::Lifecycle }
        #(#command_routes)* #(#publish)*
        #visibility struct #name { __context: #context }
        #visibility struct #builder { #(#builder_fields),* }
        impl #name { pub fn builder()->#builder { #builder { #(#empty),* } }
            pub async fn send<M>(&self,message:M)->Result<(),::actor_framework::SendError> where #context: ::actor_framework::ActorSendRoute<M> { ::actor_framework::ActorSendRoute::send_route(&self.__context,message).await }
            pub fn try_send<M>(&self,message:M)->Result<(),::actor_framework::TrySendError<M>> where #context: ::actor_framework::ActorTrySendRoute<M> { ::actor_framework::ActorTrySendRoute::try_send_route(&self.__context,message) }
            pub async fn publish<M>(&self,message:M)->Result<(),::actor_framework::SendError> where #context: ::actor_framework::ActorPublishRoute<M> { ::actor_framework::ActorPublishRoute::publish_route(&self.__context,message).await }
            /// Reject new sends, drain accepted messages, then wait for all actor tasks.
            pub async fn shutdown(&self) {
                self.__context.__lifecycle.request_shutdown();
                self.__context.__lifecycle.wait().await;
            }
        }
        impl #builder { #(#setters)*
            #[allow(non_snake_case, non_upper_case_globals)]
            pub fn start(self, spawner: ::embassy_executor::Spawner)->Result<#name,::actor_framework::embassy::StartError> {
                #(#extracts)* #(#statics)* #(#channel_init)*
                #(#validate_capacities)*
                static #lifecycle_cell: ::actor_framework::embassy::Lifecycle = ::actor_framework::embassy::Lifecycle::new(#worker_count);
                let lifecycle = &#lifecycle_cell;
                lifecycle.begin_start()?;
                let task_context = #context { #(#fields),*, __lifecycle: lifecycle };
                // Reserve every fixed task slot before spawning anything: a
                // capacity error therefore cannot leave a partially running system.
                #(#token_preparation)*
                lifecycle.commit_start();
                #(#token_spawns)*
                Ok(#name { __context: #context { #(#handle_fields),*, __lifecycle: lifecycle } })
            }
        }
        const _: () = { let _ = stringify!(#handle_sends); };
    })
}
