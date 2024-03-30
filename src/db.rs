use once_cell::sync::Lazy;
use surrealdb::engine::remote::ws::{Client, Ws};
use surrealdb::opt::auth::{Namespace, Root};
use surrealdb::Surreal;

use crate::{I_PASS, I_USER};

pub static G_DB: Lazy<Surreal<Client>> = Lazy::new(Surreal::init);
pub static I_DB: Lazy<Surreal<Client>> = Lazy::new(Surreal::init);

pub enum DbCred<'a> {
    Root(Root<'a>),
    Namespace(Namespace<'a>),
}

pub struct ConnectionCred {
    pub host: &'static str,
    pub port: u16,
    pub namespace: &'static str,
    pub database: &'static str,
    pub cred: DbCred<'static>,
}

pub async fn init_connections() -> surrealdb::Result<()> {
    init_g_db(ConnectionCred {
        host: "localhost",
        port: 8000,
        namespace: "global",
        database: "main",
        cred: DbCred::Root(Root {
            username: "root",
            password: "root",
        }),
    })
    .await?;

    init_i_db(ConnectionCred {
        host: "localhost",
        port: 8000,
        namespace: "global",
        database: "main",
        cred: DbCred::Root(Root {
            username: "root",
            password: "root",
        }),
    })
    .await?;

    Ok(())
}

async fn init_g_db(cred: ConnectionCred) -> surrealdb::Result<()> {
    G_DB.connect::<Ws>(format!("{}:{}", cred.host, cred.port))
        .await?;

    match cred.cred {
        DbCred::Root(cred) => G_DB.signin(cred).await?,
        _ => return Err(surrealdb::Error::Db(surrealdb::error::Db::InvalidAuth)),
    };

    G_DB.use_ns(cred.namespace).use_db(cred.database).await?;

    // create interventions user
    let sql = format!(
        "USE NS interventions; DEFINE USER {} ON NAMESPACE PASSWORD '{}' ROLES EDITOR;",
        I_USER, I_PASS
    );
    G_DB.query(sql).await?;

    Ok(())
}

async fn init_i_db(cred: ConnectionCred) -> surrealdb::Result<()> {
    I_DB.connect::<Ws>(format!("{}:{}", cred.host, cred.port))
        .await?;

    match cred.cred {
        // DbCred::Namespace(cred) => I_DB.signin(cred).await?,
        DbCred::Root(cred) => I_DB.signin(cred).await?,
        _ => return Err(surrealdb::Error::Db(surrealdb::error::Db::InvalidAuth)),
    };

    I_DB.use_ns("interventions").await?;

    Ok(())
}
