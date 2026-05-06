use crate::{
    domain::{
        BROADCAST_RECIPIENT, Inbox, Message, Participant, Project, SendMessage, Session,
        StartParticipant,
    },
    error::{AppError, Result},
    id, time, validation,
};
use sqlx::{PgPool, Row, postgres::PgPoolOptions};

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

impl Store {
    pub async fn connect(database_url: &str) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(10)
            .connect(database_url)
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    async fn migrate(&self) -> Result<()> {
        let statements = [
            r#"
            CREATE TABLE IF NOT EXISTS participants (
                identity TEXT PRIMARY KEY,
                role TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
            "CREATE INDEX IF NOT EXISTS idx_participants_role ON participants(role)",
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                alias TEXT PRIMARY KEY,
                root TEXT NOT NULL DEFAULT '',
                created_at TEXT NOT NULL
            )
            "#,
            r#"
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                project_alias TEXT NOT NULL REFERENCES projects(alias) ON DELETE CASCADE,
                sender_identity TEXT NOT NULL REFERENCES participants(identity),
                sender_role TEXT NOT NULL,
                recipient_kind TEXT NOT NULL CHECK (recipient_kind IN ('identity', 'role', 'broadcast')),
                recipient TEXT NOT NULL,
                subject TEXT NOT NULL,
                body TEXT NOT NULL,
                created_at TEXT NOT NULL,
                created_at_ns BIGINT NOT NULL
            )
            "#,
            "CREATE INDEX IF NOT EXISTS idx_messages_project_created ON messages(project_alias, created_at_ns, id)",
            "CREATE INDEX IF NOT EXISTS idx_messages_recipient ON messages(project_alias, recipient_kind, recipient)",
            r#"
            CREATE TABLE IF NOT EXISTS receipts (
                message_id TEXT NOT NULL REFERENCES messages(id) ON DELETE CASCADE,
                identity TEXT NOT NULL REFERENCES participants(identity) ON DELETE CASCADE,
                read_at TEXT NOT NULL,
                PRIMARY KEY(message_id, identity)
            )
            "#,
            "CREATE INDEX IF NOT EXISTS idx_receipts_identity ON receipts(identity)",
        ];
        for statement in statements {
            sqlx::query(statement).execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn start(&self, input: StartParticipant) -> Result<Session> {
        validation::role(&input.role)?;
        let role = input.role.trim().to_string();
        let identity = match input.identity {
            Some(value) if !value.trim().is_empty() => {
                validation::identity(&value)?;
                value.trim().to_string()
            }
            _ => self.allocate_identity().await?,
        };
        let (now, _) = time::now_parts();
        let mut tx = self.pool.begin().await?;
        if let Some(existing) = sqlx::query(
            "SELECT identity, role, created_at, updated_at FROM participants WHERE identity = $1",
        )
        .bind(&identity)
        .fetch_optional(&mut *tx)
        .await?
        {
            let existing_role: String = existing.get("role");
            if existing_role != role {
                return Err(AppError::Conflict(format!(
                    "identity {identity:?} is already registered as role {existing_role:?}"
                )));
            }
            sqlx::query("UPDATE participants SET updated_at = $1 WHERE identity = $2")
                .bind(&now)
                .bind(&identity)
                .execute(&mut *tx)
                .await?;
            tx.commit().await?;
            return Ok(Session { identity, role });
        }

        sqlx::query(
            "INSERT INTO participants(identity, role, created_at, updated_at) VALUES ($1, $2, $3, $4)",
        )
        .bind(&identity)
        .bind(&role)
        .bind(&now)
        .bind(&now)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(Session { identity, role })
    }

    pub async fn participant(&self, identity: &str) -> Result<Participant> {
        validation::identity(identity)?;
        let row = sqlx::query(
            "SELECT identity, role, created_at, updated_at FROM participants WHERE identity = $1",
        )
        .bind(identity)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("unknown identity {identity:?}")))?;
        Ok(participant_from_row(&row))
    }

    pub async fn participants(&self) -> Result<Vec<Participant>> {
        let rows = sqlx::query(
            "SELECT identity, role, created_at, updated_at FROM participants ORDER BY identity",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(participant_from_row).collect())
    }

    pub async fn add_project(&self, alias: &str, root: &str) -> Result<Project> {
        let alias = alias.trim();
        validation::alias(alias)?;
        let root = root.trim();
        let (now, _) = time::now_parts();
        sqlx::query(
            r#"
            INSERT INTO projects(alias, root, created_at) VALUES ($1, $2, $3)
            ON CONFLICT(alias) DO UPDATE SET root = EXCLUDED.root
            "#,
        )
        .bind(alias)
        .bind(root)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        self.project(alias).await
    }

    pub async fn project(&self, alias: &str) -> Result<Project> {
        validation::alias(alias)?;
        let row = sqlx::query("SELECT alias, root, created_at FROM projects WHERE alias = $1")
            .bind(alias)
            .fetch_optional(&self.pool)
            .await?
            .ok_or_else(|| AppError::NotFound(format!("unknown project {alias:?}")))?;
        Ok(project_from_row(&row))
    }

    pub async fn projects(&self) -> Result<Vec<Project>> {
        let rows = sqlx::query("SELECT alias, root, created_at FROM projects ORDER BY alias")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(project_from_row).collect())
    }

    pub async fn send(&self, input: SendMessage) -> Result<Message> {
        validation::alias(&input.project)?;
        let sender = self.participant(&input.sender_identity).await?;
        self.project(&input.project).await?;
        let (kind, recipient) = self.normalize_recipient(&input.to_kind, &input.to).await?;
        let subject = input.subject.trim();
        if subject.is_empty() {
            return Err(AppError::BadRequest("subject is required".into()));
        }
        let id = id::message_id().map_err(|err| AppError::BadRequest(err.to_string()))?;
        let (created_at, created_at_ns) = time::now_parts();
        let msg = Message {
            id,
            project: input.project,
            sender_identity: sender.identity,
            sender_role: sender.role,
            recipient_kind: kind,
            recipient,
            subject: subject.to_string(),
            body: input.body,
            created_at,
            created_at_ns,
            read_at: String::new(),
        };
        sqlx::query(
            r#"
            INSERT INTO messages(id, project_alias, sender_identity, sender_role, recipient_kind, recipient, subject, body, created_at, created_at_ns)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(&msg.id)
        .bind(&msg.project)
        .bind(&msg.sender_identity)
        .bind(&msg.sender_role)
        .bind(&msg.recipient_kind)
        .bind(&msg.recipient)
        .bind(&msg.subject)
        .bind(&msg.body)
        .bind(&msg.created_at)
        .bind(msg.created_at_ns)
        .execute(&self.pool)
        .await?;
        Ok(msg)
    }

    pub async fn inbox(&self, project: &str, identity: &str) -> Result<Inbox> {
        validation::alias(project)?;
        let participant = self.participant(identity).await?;
        self.project(project).await?;
        let rows = sqlx::query(
            r#"
            SELECT m.id, m.project_alias, m.sender_identity, m.sender_role, m.recipient_kind, m.recipient, m.subject, '' AS body, m.created_at, m.created_at_ns, '' AS read_at
            FROM messages m
            LEFT JOIN receipts r ON r.message_id = m.id AND r.identity = $1
            WHERE m.project_alias = $2
              AND r.message_id IS NULL
              AND (
                (m.recipient_kind = 'identity' AND m.recipient = $1) OR
                (m.recipient_kind = 'role' AND m.recipient = $3) OR
                m.recipient_kind = 'broadcast'
              )
            ORDER BY m.created_at_ns ASC, m.id ASC
            "#,
        )
        .bind(&participant.identity)
        .bind(project)
        .bind(&participant.role)
        .fetch_all(&self.pool)
        .await?;
        let messages: Vec<Message> = rows.iter().map(message_from_row).collect();
        Ok(Inbox {
            project: project.to_string(),
            identity: participant.identity,
            role: participant.role,
            unread_count: messages.len(),
            messages,
        })
    }

    pub async fn mark_read(&self, project: &str, mail_id: &str, identity: &str) -> Result<()> {
        self.delivered_message(project, mail_id, identity).await?;
        let (now, _) = time::now_parts();
        sqlx::query(
            r#"
            INSERT INTO receipts(message_id, identity, read_at) VALUES ($1, $2, $3)
            ON CONFLICT(message_id, identity) DO UPDATE SET read_at = EXCLUDED.read_at
            "#,
        )
        .bind(mail_id)
        .bind(identity)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn message(&self, project: &str, mail_id: &str, identity: &str) -> Result<Message> {
        self.delivered_message(project, mail_id, identity).await
    }

    async fn delivered_message(
        &self,
        project: &str,
        mail_id: &str,
        identity: &str,
    ) -> Result<Message> {
        validation::alias(project)?;
        let participant = self.participant(identity).await?;
        let row = sqlx::query(
            r#"
            SELECT m.id, m.project_alias, m.sender_identity, m.sender_role, m.recipient_kind, m.recipient, m.subject, m.body, m.created_at, m.created_at_ns, COALESCE(r.read_at, '') AS read_at
            FROM messages m
            LEFT JOIN receipts r ON r.message_id = m.id AND r.identity = $1
            WHERE m.project_alias = $2
              AND m.id = $3
              AND (
                (m.recipient_kind = 'identity' AND m.recipient = $1) OR
                (m.recipient_kind = 'role' AND m.recipient = $4) OR
                m.recipient_kind = 'broadcast'
              )
            "#,
        )
        .bind(identity)
        .bind(project)
        .bind(mail_id)
        .bind(&participant.role)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::NotFound("message not found".into()))?;
        Ok(message_from_row(&row))
    }

    async fn normalize_recipient(&self, kind: &str, to: &str) -> Result<(String, String)> {
        let kind = kind.trim();
        let to = to.trim();
        if to.is_empty() {
            return Err(AppError::BadRequest("recipient is required".into()));
        }
        if kind.is_empty() {
            if to == BROADCAST_RECIPIENT {
                return Ok(("broadcast".into(), to.into()));
            }
            validation::recipient(to)?;
            return match self.participant(to).await {
                Ok(_) => Ok(("identity".into(), to.into())),
                Err(AppError::NotFound(_)) => Ok(("role".into(), to.into())),
                Err(err) => Err(err),
            };
        }
        match kind {
            "broadcast" => {
                if to != BROADCAST_RECIPIENT {
                    return Err(AppError::BadRequest(
                        "broadcast recipient must be all-agents".into(),
                    ));
                }
                Ok((kind.into(), to.into()))
            }
            "identity" => {
                self.participant(to).await?;
                Ok((kind.into(), to.into()))
            }
            "role" => {
                validation::role(to)?;
                Ok((kind.into(), to.into()))
            }
            _ => Err(AppError::BadRequest(
                "recipient kind must be identity, role, or broadcast".into(),
            )),
        }
    }

    async fn allocate_identity(&self) -> Result<String> {
        for _ in 0..20 {
            let candidate = id::identity().map_err(|err| AppError::BadRequest(err.to_string()))?;
            if self.participant(&candidate).await.is_err() {
                return Ok(candidate);
            }
        }
        Err(AppError::Conflict(
            "could not allocate unique identity".into(),
        ))
    }
}

fn participant_from_row(row: &sqlx::postgres::PgRow) -> Participant {
    Participant {
        identity: row.get("identity"),
        role: row.get("role"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    }
}

fn project_from_row(row: &sqlx::postgres::PgRow) -> Project {
    Project {
        alias: row.get("alias"),
        root: row.get("root"),
        created_at: row.get("created_at"),
    }
}

fn message_from_row(row: &sqlx::postgres::PgRow) -> Message {
    Message {
        id: row.get("id"),
        project: row.get("project_alias"),
        sender_identity: row.get("sender_identity"),
        sender_role: row.get("sender_role"),
        recipient_kind: row.get("recipient_kind"),
        recipient: row.get("recipient"),
        subject: row.get("subject"),
        body: row.get("body"),
        created_at: row.get("created_at"),
        created_at_ns: row.get("created_at_ns"),
        read_at: row.get("read_at"),
    }
}
