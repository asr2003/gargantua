use blueprint::{Blueprint, Graph};
use futures::future;
use query_plan::{Fetch, QueryPlan, SelectionSet, TypeName};

// TODO: implement reference implementation
pub trait ResolverContextTrait {
    fn plan(&self) -> &QueryPlan<serde_json::Value>;
    fn with_plan(&self, plan: QueryPlan<serde_json::Value>) -> Self;

    fn blueprint(&self) -> &Blueprint;
    fn with_blueprint(&self, blueprint: Blueprint) -> Self;

    fn value(&self) -> &serde_json::Value;
    fn value_owned(self) -> serde_json::Value;
    fn with_value(&self, value: serde_json::Value) -> Self;

    fn http(&self) -> &impl HttpIoTrait;
    fn with_http(&self, http: dyn HttpIoTrait) -> Self;
}

#[async_trait::async_trait]
pub trait HttpIoTrait {
    // TODO: implement reference implementation
    async fn execute(&self, req: reqwest::Request) -> anyhow::Result<reqwest::Response>;
}

pub async fn resolve<Ctx: ResolverContextTrait + Clone>(
    ctx: Ctx,
) -> anyhow::Result<serde_json::Value> {
    let plan = ctx.plan().clone();
    let blueprint = ctx.blueprint();

    let json_value: serde_json::Value = match plan {
        QueryPlan::Parallel(vec) => {
            let mut futures = Vec::new();

            for sub_plan in vec {
                let sub_ctx = ctx.with_plan(sub_plan);
                let future = resolve(sub_ctx);
                futures.push(future);
            }

            let results = future::join_all(futures)
                .await
                .into_iter()
                .collect::<anyhow::Result<Vec<_>>>()?;

            results
                .into_iter()
                .fold(ctx.value_owned(), |value, other_value| {
                    merge(value, other_value)
                })
        }
        QueryPlan::Sequence(vec) => {
            let mut value = ctx.value().clone();

            for sub_plan in vec {
                let other_ctx = ctx.with_plan(sub_plan);
                let other_value = resolve(other_ctx).await?;
                value = merge(value, other_value);
            }

            value
        }
        QueryPlan::Fetch(Fetch { service, selection_set, representations, type_name, .. }) => {
            let req = prepare_req(
                blueprint,
                &service,
                &selection_set,
                &representations,
                type_name,
            );

            let res: serde_json::Value = ctx.http().execute(req).await?.json().await?;

            // TODO: select only requested fields from res

            res
        }
        QueryPlan::Flatten { select, plan } => {
            let path_value = select.get(ctx.value().clone());

            let path_ctx = ctx.with_value(path_value).with_plan(*plan);

            let other_value = resolve(path_ctx).await?;

            select.set(ctx.value_owned(), other_value)
        }
    };

    Ok(json_value)
}

fn merge(value: serde_json::Value, other_value: serde_json::Value) -> serde_json::Value {
    match (value, other_value) {
        (serde_json::Value::Object(mut a), serde_json::Value::Object(b)) => {
            for (key, b_value) in b.into_iter() {
                if let Some(a_value) = a.get_mut(&key) {
                    *a_value = merge(a_value.clone(), b_value);
                } else {
                    a.insert(key, b_value);
                }
            }
            serde_json::Value::Object(a)
        }
        (serde_json::Value::Array(mut a), serde_json::Value::Array(b)) => {
            let mut new_array = Vec::with_capacity(a.len() + b.len());
            new_array.append(&mut a);
            new_array.extend_from_slice(&b);
            serde_json::Value::Array(new_array)
        }
        (_, other_value) => other_value,
    }
}

fn prepare_req<Value>(
    _blueprint: &Blueprint,
    _service: &Option<Graph>,
    _query: &SelectionSet<Value>,
    _representations: &Option<SelectionSet<Value>>,
    _type_name: TypeName,
) -> reqwest::Request {
    // TODO: prepare query string
    // TODO: prepare request
    todo!()
}
