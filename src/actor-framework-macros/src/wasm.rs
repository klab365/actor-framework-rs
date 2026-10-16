use super::*;

pub(super) fn expand(
    visibility: syn::Visibility,
    name: Ident,
    actors: Vec<ActorDef>,
    events: Vec<Type>,
) -> proc_macro2::TokenStream {
    let context = format_ident!("__{}Context", name);
    let builder = format_ident!("{}Builder", name);
    let mut enums = Vec::new();
    let mut fields = Vec::new();
    let mut initial = Vec::new();
    let mut builder_fields = Vec::new();
    let mut empty = Vec::new();
    let mut setters = Vec::new();
    let mut extracts = Vec::new();
    let mut channels = Vec::new();
    let mut spawns = Vec::new();
    let mut closes = Vec::new();
    let mut routes = Vec::new();
    let mut event_groups: Vec<(Type, Vec<proc_macro2::TokenStream>)> =
        events.into_iter().map(|ty| (ty, Vec::new())).collect();

    for actor in actors {
        let field = actor.field;
        let state = actor.state;
        let capacity = actor.capacity;
        let message_enum = format_ident!("__{}MailboxMessage", field);
        let receiver = format_ident!("__{}_receiver", field);
        let state_var = format_ident!("__{}_state", field);
        let mut variants = Vec::new();
        let mut arms = Vec::new();
        let command_count = actor.commands.len();
        for (index, route) in actor.commands.into_iter().chain(actor.events).enumerate() {
            let ty = route.message;
            let handler = route.handler;
            let variant = format_ident!("Message{}", index);
            variants.push(quote!(#variant(#ty)));
            arms.push(quote!(#message_enum::#variant(message) => {
                let () = #handler(&mut #state_var, message).await;
            }));
            if index < command_count {
                routes.push(quote! {
                    impl ::actor_framework::ActorSendRoute<#ty> for #context {
                        async fn send_route(&self, message: #ty) -> Result<(), ::actor_framework::SendError> {
                            ::actor_framework::ActorSendRoute::send_route(&self.#field, #message_enum::#variant(message)).await
                        }
                    }
                    impl __ActorTrySendRoute<#ty> for #context {
                        fn try_route(&self, message: #ty) -> Result<(), ::actor_framework::TrySendError<#ty>> {
                            match self.#field.try_send(#message_enum::#variant(message)) {
                                Ok(()) => Ok(()),
                                Err(::actor_framework::TrySendError::Full(#message_enum::#variant(message))) => Err(::actor_framework::TrySendError::Full(message)),
                                Err(::actor_framework::TrySendError::Closed(#message_enum::#variant(message))) => Err(::actor_framework::TrySendError::Closed(message)),
                                _ => unreachable!("the attempted variant is preserved"),
                            }
                        }
                    }
                });
            } else {
                let key = quote!(#ty).to_string();
                if !event_groups.iter().any(|(ty, _)| quote!(#ty).to_string() == key) {
                    event_groups.push((ty.clone(), Vec::new()));
                }
                let (_, sends) = event_groups
                    .iter_mut()
                    .find(|(ty, _)| quote!(#ty).to_string() == key)
                    .unwrap();
                sends.push(quote! {
                    ::actor_framework::ActorSendRoute::send_route(&self.#field, #message_enum::#variant(message.clone())).await?;
                });
            }
        }
        enums.push(quote!(#[allow(non_camel_case_types)] enum #message_enum { #(#variants),* }));
        fields.push(quote!(#field: ::actor_framework::wasm::Sender<#message_enum>));
        initial.push(quote!(#field));
        builder_fields.push(quote!(#field: Option<#state>));
        empty.push(quote!(#field: None));
        setters.push(quote!(pub fn #field(mut self, state: #state) -> Self { self.#field = Some(state); self }));
        extracts.push(quote! {
            let mut #state_var = self.#field.ok_or(::actor_framework::wasm::StartError::MissingActorState(stringify!(#field)))?;
        });
        channels.push(quote! {
            let (#field, mut #receiver) = ::actor_framework::wasm::channel::<#message_enum>(#capacity)
                .map_err(|_| ::actor_framework::wasm::StartError::InvalidCapacity(stringify!(#field)))?;
        });
        closes.push(quote!(self.__context.#field.close();));
        spawns.push(quote! {
            let task_context = system.__context.clone();
            let done = system.__done_tx.clone();
            ::actor_framework::wasm::__spawn_local(async move {
                while let Some(message) = ::actor_framework::Receiver::receive(&mut #receiver).await {
                    match message { #(#arms),* }
                }
                let _ = ::actor_framework::ActorSendRoute::send_route(&done, ()).await;
            });
        });
    }
    for (ty, sends) in event_groups {
        routes.push(quote! {
            impl ::actor_framework::ActorPublishRoute<#ty> for #context {
                async fn publish_route(&self, message: #ty) -> Result<(), ::actor_framework::SendError> {
                    #(#sends)*
                    Ok(())
                }
            }
        });
    }
    let count = spawns.len();
    quote! {
        #(#enums)*
        #[doc(hidden)]
        pub trait __ActorTrySendRoute<M> {
            fn try_route(&self, message: M) -> Result<(), ::actor_framework::TrySendError<M>>;
        }
        #[derive(Clone)]
        struct #context { #(#fields),* }
        #visibility struct #name {
            __context: #context,
            __done_tx: ::actor_framework::wasm::Sender<()>,
            __done_rx: ::std::cell::RefCell<::actor_framework::wasm::Receiver<()>>,
            __remaining: ::std::cell::Cell<usize>,
        }
        #visibility struct #builder { #(#builder_fields),* }
        #(#routes)*
        impl Drop for #name {
            fn drop(&mut self) { #(#closes)* }
        }
        impl #name {
            pub fn builder() -> #builder { #builder { #(#empty),* } }
            pub async fn send<M>(&self, message: M) -> Result<(), ::actor_framework::SendError>
            where #context: ::actor_framework::ActorSendRoute<M> {
                ::actor_framework::ActorSendRoute::send_route(&self.__context, message).await
            }
            pub fn try_send<M>(&self, message: M) -> Result<(), ::actor_framework::TrySendError<M>>
            where #context: __ActorTrySendRoute<M> {
                __ActorTrySendRoute::try_route(&self.__context, message)
            }
            pub async fn publish<M>(&self, message: M) -> Result<(), ::actor_framework::SendError>
            where #context: ::actor_framework::ActorPublishRoute<M> {
                ::actor_framework::ActorPublishRoute::publish_route(&self.__context, message).await
            }
            /// Close all mailboxes and drain accepted messages. Concurrent calls
            /// return ShutdownInProgress; cancellation permits a later retry.
            /// Panics are not recovered and can prevent shutdown completion.
            // Intentional local-only exclusion: competing calls use try_borrow_mut
            // and return an error, never panic or block. Dropping the future releases it.
            #[allow(clippy::await_holding_refcell_ref)]
            pub async fn shutdown(&self) -> Result<(), ::actor_framework::wasm::ShutdownInProgress> {
                #(#closes)*
                let mut receiver = self.__done_rx.try_borrow_mut()
                    .map_err(|_| ::actor_framework::wasm::ShutdownInProgress)?;
                while self.__remaining.get() > 0 {
                    ::actor_framework::Receiver::receive(&mut *receiver).await;
                    self.__remaining.set(self.__remaining.get() - 1);
                }
                Ok(())
            }
        }
        impl #builder {
            #(#setters)*
            pub fn start(self) -> Result<#name, ::actor_framework::wasm::StartError> {
                #(#extracts)*
                #(#channels)*
                let (done_tx, done_rx) = ::actor_framework::wasm::channel(#count).expect("nonempty actor system");
                let system = #name {
                    __context: #context { #(#initial),* },
                    __done_tx: done_tx,
                    __done_rx: ::std::cell::RefCell::new(done_rx),
                    __remaining: ::std::cell::Cell::new(#count),
                };
                #(#spawns)*
                Ok(system)
            }
        }
    }
}
