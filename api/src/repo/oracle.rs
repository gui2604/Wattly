use oracle::pool::PoolBuilder;
use oracle::Connection;

use crate::config::{self, Config};
use crate::domain::models::{
    CheckRecord, ClientRecord, DocumentRecord, EventRecord, LocationRecord, UserRecord,
    WindowRecord,
};
use crate::error::AppError;

use super::Repository;

pub struct OracleRepo {
    pool: oracle::pool::Pool,
}

impl OracleRepo {
    pub fn connect(config: &Config) -> Result<Self, AppError> {
        let mut builder = PoolBuilder::new(
            config.oracle_user.clone(),
            config.oracle_password.clone(),
            config.oracle_connect.clone(),
        );
        builder
            .min_connections(1)
            .max_connections(8)
            .connection_increment(1);
        let pool = builder.build().map_err(map_oracle)?;
        let conn = pool.get().map_err(map_oracle)?;
        ensure_schema(&conn).map_err(map_oracle)?;
        conn.commit().map_err(map_oracle)?;
        Ok(Self { pool })
    }

    fn with_conn<T>(
        &self,
        work: impl FnOnce(&Connection) -> Result<T, oracle::Error>,
    ) -> Result<T, AppError> {
        let conn = self.pool.get().map_err(map_oracle)?;
        match work(&conn) {
            Ok(value) => {
                conn.commit().map_err(map_oracle)?;
                Ok(value)
            }
            Err(err) => {
                let _ = conn.rollback();
                Err(map_oracle(err))
            }
        }
    }
}

impl Repository for OracleRepo {
    fn ping(&self) -> Result<String, AppError> {
        self.with_conn(|conn| conn.query_row_as::<String>("SELECT 'ok' FROM dual", &[]))
    }

    fn create_user(&self, user: UserRecord) -> Result<(), AppError> {
        self.with_conn(|conn| insert_user(conn, &user))
    }

    fn find_user_by_email(&self, email: &str) -> Result<Option<UserRecord>, AppError> {
        self.with_conn(|conn| find_user(conn, "EMAIL = :1", email))
    }

    fn find_user_by_id(&self, id: &str) -> Result<Option<UserRecord>, AppError> {
        self.with_conn(|conn| find_user(conn, "ID = :1", id))
    }

    fn set_last_login(&self, id: &str, at: &str) -> Result<(), AppError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE WATTLY_USERS SET LAST_LOGIN = :1, UPDATED_AT = :2 WHERE ID = :3",
                &[&at, &at, &id],
            )?;
            Ok(())
        })
    }

    fn set_user_status(&self, id: &str, status: &str, at: &str) -> Result<(), AppError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE WATTLY_USERS SET STATUS = :1, UPDATED_AT = :2 WHERE ID = :3",
                &[&status, &at, &id],
            )?;
            Ok(())
        })
    }

    fn create_client(&self, client: ClientRecord) -> Result<(), AppError> {
        self.with_conn(|conn| insert_client(conn, &client))
    }

    fn update_client(&self, client: ClientRecord) -> Result<(), AppError> {
        self.with_conn(|conn| update_client(conn, &client))
    }

    fn find_client(&self, id: &str) -> Result<Option<ClientRecord>, AppError> {
        self.with_conn(|conn| {
            find_one(
                conn,
                "SELECT * FROM WATTLY_CLIENTS WHERE ID = :1",
                id,
                map_client,
            )
        })
    }

    fn list_clients_by_owner(&self, owner_id: &str) -> Result<Vec<ClientRecord>, AppError> {
        self.with_conn(|conn| {
            list(
                conn,
                "SELECT * FROM WATTLY_CLIENTS WHERE OWNER_ID = :1 ORDER BY CREATED_AT DESC",
                owner_id,
                map_client,
            )
        })
    }

    fn list_clients(&self) -> Result<Vec<ClientRecord>, AppError> {
        self.with_conn(|conn| {
            let rows = conn.query("SELECT * FROM WATTLY_CLIENTS ORDER BY CREATED_AT DESC", &[])?;
            collect(rows, map_client)
        })
    }

    fn create_location(
        &self,
        location: LocationRecord,
        windows: Vec<WindowRecord>,
    ) -> Result<(), AppError> {
        self.with_conn(|conn| {
            insert_location(conn, &location)?;
            for window in &windows {
                insert_window(conn, window)?;
            }
            Ok(())
        })
    }

    fn list_locations_by_client(
        &self,
        client_id: &str,
    ) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        self.with_conn(|conn| locations_where(conn, "L.CLIENT_ID = :1", Some(client_id)))
    }

    fn find_location(
        &self,
        id: &str,
    ) -> Result<Option<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        self.with_conn(|conn| {
            let rows = locations_where(conn, "L.ID = :1", Some(id))?;
            Ok(rows.into_iter().next())
        })
    }

    fn list_public_locations(&self) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, AppError> {
        self.with_conn(|conn| {
            locations_where(conn, "C.STATUS = 'approved' AND L.STATUS = 'active'", None)
        })
    }

    fn set_locations_status(
        &self,
        client_id: &str,
        status: &str,
        at: &str,
    ) -> Result<(), AppError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE WATTLY_LOCATIONS SET STATUS = :1, UPDATED_AT = :2 WHERE CLIENT_ID = :3",
                &[&status, &at, &client_id],
            )?;
            Ok(())
        })
    }

    fn add_document(&self, document: DocumentRecord) -> Result<(), AppError> {
        self.with_conn(|conn| insert_document(conn, &document))
    }

    fn list_documents(&self, client_id: &str) -> Result<Vec<DocumentRecord>, AppError> {
        self.with_conn(|conn| {
            list(
                conn,
                "SELECT * FROM WATTLY_DOCUMENTS WHERE CLIENT_ID = :1 ORDER BY CREATED_AT",
                client_id,
                map_document,
            )
        })
    }

    fn add_checks(&self, checks: Vec<CheckRecord>) -> Result<(), AppError> {
        self.with_conn(|conn| {
            for check in &checks {
                insert_check(conn, check)?;
            }
            Ok(())
        })
    }

    fn list_checks(&self, client_id: &str) -> Result<Vec<CheckRecord>, AppError> {
        self.with_conn(|conn| {
            list(
                conn,
                "SELECT * FROM WATTLY_CHECKS WHERE CLIENT_ID = :1 ORDER BY CREATED_AT",
                client_id,
                map_check,
            )
        })
    }

    fn add_event(&self, event: EventRecord) -> Result<(), AppError> {
        self.with_conn(|conn| insert_event(conn, &event))
    }

    fn list_events(&self, client_id: &str) -> Result<Vec<EventRecord>, AppError> {
        self.with_conn(|conn| {
            list(
                conn,
                "SELECT * FROM WATTLY_CRED_EVENTS WHERE CLIENT_ID = :1 ORDER BY CREATED_AT",
                client_id,
                map_event,
            )
        })
    }
}

fn find_user(
    conn: &Connection,
    filter: &str,
    value: &str,
) -> Result<Option<UserRecord>, oracle::Error> {
    let sql = format!("SELECT * FROM WATTLY_USERS WHERE {filter}");
    find_one(conn, &sql, value, map_user)
}

fn locations_where(
    conn: &Connection,
    filter: &str,
    value: Option<&str>,
) -> Result<Vec<(LocationRecord, Vec<WindowRecord>)>, oracle::Error> {
    let sql = format!(
        "SELECT L.* FROM WATTLY_LOCATIONS L
         JOIN WATTLY_CLIENTS C ON C.ID = L.CLIENT_ID
         WHERE {filter}
         ORDER BY L.CREATED_AT"
    );
    let rows = match value {
        Some(value) => conn.query(&sql, &[&value])?,
        None => conn.query(&sql, &[])?,
    };
    let locations = collect(rows, map_location)?;
    let mut paired = Vec::with_capacity(locations.len());
    for location in locations {
        let windows = list(
            conn,
            "SELECT * FROM WATTLY_WINDOWS WHERE LOCATION_ID = :1 ORDER BY WEEKDAY, START_MIN",
            &location.id,
            map_window,
        )?;
        paired.push((location, windows));
    }
    Ok(paired)
}

fn find_one<T>(
    conn: &Connection,
    sql: &str,
    value: &str,
    map: fn(&oracle::Row) -> Result<T, oracle::Error>,
) -> Result<Option<T>, oracle::Error> {
    match conn.query_row(sql, &[&value]) {
        Ok(row) => Ok(Some(map(&row)?)),
        Err(err) if err.kind() == oracle::ErrorKind::NoDataFound => Ok(None),
        Err(err) => Err(err),
    }
}

fn list<T>(
    conn: &Connection,
    sql: &str,
    value: &str,
    map: fn(&oracle::Row) -> Result<T, oracle::Error>,
) -> Result<Vec<T>, oracle::Error> {
    let rows = conn.query(sql, &[&value])?;
    collect(rows, map)
}

fn collect<T>(
    rows: oracle::ResultSet<'_, oracle::Row>,
    map: fn(&oracle::Row) -> Result<T, oracle::Error>,
) -> Result<Vec<T>, oracle::Error> {
    let mut items = Vec::new();
    for row in rows {
        items.push(map(&row?)?);
    }
    Ok(items)
}

fn text(value: &str) -> Option<&str> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn insert_user(conn: &Connection, user: &UserRecord) -> Result<(), oracle::Error> {
    conn.execute(
        "INSERT INTO WATTLY_USERS (
            ID, EMAIL, PASSWORD_HASH, FULL_NAME, PHONE, DOC_NUMBER, ROLE_NAME, STATUS,
            TERMS_VER, CONSENT_AT, CREATED_AT, UPDATED_AT
         ) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12)",
        &[
            &user.id,
            &user.email,
            &user.password_hash,
            &user.full_name,
            &user.phone,
            &user.document,
            &user.role,
            &user.status,
            &user.terms_version,
            &user.consent_at,
            &user.created_at,
            &user.updated_at,
        ],
    )?;
    Ok(())
}

fn insert_client(conn: &Connection, client: &ClientRecord) -> Result<(), oracle::Error> {
    let bio = text(&client.bio);
    conn.execute(
        "INSERT INTO WATTLY_CLIENTS (
            ID, OWNER_ID, TRADE_NAME, LEGAL_NAME, PERSON_TYPE, TAX_ID, PIX_TYPE, PIX_KEY, BIO,
            STATUS, CREATED_AT, UPDATED_AT
         ) VALUES (:1, :2, :3, :4, :5, :6, :7, :8, :9, :10, :11, :12)",
        &[
            &client.id as &dyn oracle::sql_type::ToSql,
            &client.owner_id,
            &client.trade_name,
            &client.legal_name,
            &client.person_type,
            &client.tax_id,
            &client.pix_key_type,
            &client.pix_key,
            &bio,
            &client.status,
            &client.created_at,
            &client.updated_at,
        ],
    )?;
    Ok(())
}

fn update_client(conn: &Connection, client: &ClientRecord) -> Result<(), oracle::Error> {
    let bio = text(&client.bio);
    conn.execute(
        "UPDATE WATTLY_CLIENTS SET
            TRADE_NAME = :1, LEGAL_NAME = :2, BIO = :3, STATUS = :4,
            REJECT_REASON = :5, DECISION_NOTE = :6, SUBMITTED_AT = :7, REVIEWED_AT = :8,
            REVIEWER_ID = :9, UPDATED_AT = :10
         WHERE ID = :11",
        &[
            &client.trade_name as &dyn oracle::sql_type::ToSql,
            &client.legal_name,
            &bio,
            &client.status,
            &client.reject_reason,
            &client.decision_note,
            &client.submitted_at,
            &client.reviewed_at,
            &client.reviewer_id,
            &client.updated_at,
            &client.id,
        ],
    )?;
    Ok(())
}

fn insert_location(conn: &Connection, location: &LocationRecord) -> Result<(), oracle::Error> {
    let description = text(&location.description);
    let has_roof = i32::from(location.has_roof);
    let well_lit = i32::from(location.well_lit);
    conn.execute(
        "INSERT INTO WATTLY_LOCATIONS (
            ID, CLIENT_ID, LABEL, DESCRIPTION, ADDR_LINE, ADDR_NUMBER, COMPLEMENT, DISTRICT,
            CITY, STATE_CODE, POSTAL_CODE, COUNTRY_CODE, LATITUDE, LONGITUDE, PUB_LAT, PUB_LNG,
            CONNECTOR, POWER_KW, VOLTAGE, PHASES, PRICE_MIN_CENTS, PRICE_HOUR_CENTS,
            INSTALL_TYPE, HAS_ROOF, WELL_LIT, ACCESS_NOTE, SPOT_CODE, STATUS, CREATED_AT, UPDATED_AT
         ) VALUES (
            :1, :2, :3, :4, :5, :6, :7, :8,
            :9, :10, :11, :12, :13, :14, :15, :16,
            :17, :18, :19, :20, :21, :22,
            :23, :24, :25, :26, :27, :28, :29, :30
         )",
        &[
            &location.id as &dyn oracle::sql_type::ToSql,
            &location.client_id,
            &location.label,
            &description,
            &location.address_line,
            &location.address_number,
            &location.complement,
            &location.district,
            &location.city,
            &location.state,
            &location.postal_code,
            &location.country,
            &location.latitude,
            &location.longitude,
            &location.public_latitude,
            &location.public_longitude,
            &location.connector,
            &location.power_kw,
            &location.voltage,
            &location.phases,
            &location.price_per_minute_cents,
            &location.price_per_hour_cents,
            &location.install_type,
            &has_roof,
            &well_lit,
            &location.access_note,
            &location.spot_code,
            &location.status,
            &location.created_at,
            &location.updated_at,
        ],
    )?;
    Ok(())
}

fn insert_window(conn: &Connection, window: &WindowRecord) -> Result<(), oracle::Error> {
    conn.execute(
        "INSERT INTO WATTLY_WINDOWS (ID, LOCATION_ID, WEEKDAY, START_MIN, END_MIN)
         VALUES (:1, :2, :3, :4, :5)",
        &[
            &window.id,
            &window.location_id,
            &window.weekday,
            &window.start_min,
            &window.end_min,
        ],
    )?;
    Ok(())
}

fn insert_document(conn: &Connection, document: &DocumentRecord) -> Result<(), oracle::Error> {
    conn.execute(
        "INSERT INTO WATTLY_DOCUMENTS (ID, CLIENT_ID, DOC_KIND, STORAGE_REF, STATUS, NOTE, CREATED_AT)
         VALUES (:1, :2, :3, :4, :5, :6, :7)",
        &[
            &document.id as &dyn oracle::sql_type::ToSql,
            &document.client_id,
            &document.kind,
            &document.storage_ref,
            &document.status,
            &document.note,
            &document.created_at,
        ],
    )?;
    Ok(())
}

fn insert_check(conn: &Connection, check: &CheckRecord) -> Result<(), oracle::Error> {
    let passed = i32::from(check.passed);
    conn.execute(
        "INSERT INTO WATTLY_CHECKS (ID, CLIENT_ID, ITEM_CODE, PASSED, NOTE, REVIEWER_ID, CREATED_AT)
         VALUES (:1, :2, :3, :4, :5, :6, :7)",
        &[
            &check.id as &dyn oracle::sql_type::ToSql,
            &check.client_id,
            &check.item,
            &passed,
            &check.note,
            &check.reviewer_id,
            &check.created_at,
        ],
    )?;
    Ok(())
}

fn insert_event(conn: &Connection, event: &EventRecord) -> Result<(), oracle::Error> {
    let payload = text(&event.payload);
    conn.execute(
        "INSERT INTO WATTLY_CRED_EVENTS (
            ID, CLIENT_ID, ACTOR_ID, EVENT_TYPE, FROM_STATUS, TO_STATUS, PAYLOAD, CREATED_AT
         ) VALUES (:1, :2, :3, :4, :5, :6, :7, :8)",
        &[
            &event.id as &dyn oracle::sql_type::ToSql,
            &event.client_id,
            &event.actor_id,
            &event.event_type,
            &event.from_status,
            &event.to_status,
            &payload,
            &event.created_at,
        ],
    )?;
    Ok(())
}

fn map_user(row: &oracle::Row) -> Result<UserRecord, oracle::Error> {
    Ok(UserRecord {
        id: row.get("ID")?,
        email: row.get("EMAIL")?,
        password_hash: row.get("PASSWORD_HASH")?,
        full_name: row.get("FULL_NAME")?,
        phone: row.get("PHONE")?,
        document: row.get("DOC_NUMBER")?,
        role: row.get("ROLE_NAME")?,
        status: row.get("STATUS")?,
        terms_version: row.get("TERMS_VER")?,
        consent_at: row.get("CONSENT_AT")?,
        created_at: row.get("CREATED_AT")?,
        updated_at: row.get("UPDATED_AT")?,
        last_login_at: row.get("LAST_LOGIN")?,
    })
}

fn map_client(row: &oracle::Row) -> Result<ClientRecord, oracle::Error> {
    Ok(ClientRecord {
        id: row.get("ID")?,
        owner_id: row.get("OWNER_ID")?,
        trade_name: row.get("TRADE_NAME")?,
        legal_name: row.get("LEGAL_NAME")?,
        person_type: row.get("PERSON_TYPE")?,
        tax_id: row.get("TAX_ID")?,
        pix_key_type: row.get("PIX_TYPE")?,
        pix_key: row.get("PIX_KEY")?,
        bio: optional_text(row, "BIO")?,
        status: row.get("STATUS")?,
        reject_reason: row.get("REJECT_REASON")?,
        decision_note: row.get("DECISION_NOTE")?,
        submitted_at: row.get("SUBMITTED_AT")?,
        reviewed_at: row.get("REVIEWED_AT")?,
        reviewer_id: row.get("REVIEWER_ID")?,
        created_at: row.get("CREATED_AT")?,
        updated_at: row.get("UPDATED_AT")?,
    })
}

fn map_location(row: &oracle::Row) -> Result<LocationRecord, oracle::Error> {
    let has_roof: i32 = row.get("HAS_ROOF")?;
    let well_lit: i32 = row.get("WELL_LIT")?;
    Ok(LocationRecord {
        id: row.get("ID")?,
        client_id: row.get("CLIENT_ID")?,
        label: row.get("LABEL")?,
        description: optional_text(row, "DESCRIPTION")?,
        address_line: row.get("ADDR_LINE")?,
        address_number: row.get("ADDR_NUMBER")?,
        complement: row.get("COMPLEMENT")?,
        district: row.get("DISTRICT")?,
        city: row.get("CITY")?,
        state: row.get("STATE_CODE")?,
        postal_code: row.get("POSTAL_CODE")?,
        country: row.get("COUNTRY_CODE")?,
        latitude: row.get("LATITUDE")?,
        longitude: row.get("LONGITUDE")?,
        public_latitude: row.get("PUB_LAT")?,
        public_longitude: row.get("PUB_LNG")?,
        connector: row.get("CONNECTOR")?,
        power_kw: row.get("POWER_KW")?,
        voltage: row.get("VOLTAGE")?,
        phases: row.get("PHASES")?,
        price_per_minute_cents: row.get("PRICE_MIN_CENTS")?,
        price_per_hour_cents: row.get("PRICE_HOUR_CENTS")?,
        install_type: row.get("INSTALL_TYPE")?,
        has_roof: has_roof != 0,
        well_lit: well_lit != 0,
        access_note: row.get("ACCESS_NOTE")?,
        spot_code: row.get("SPOT_CODE")?,
        status: row.get("STATUS")?,
        created_at: row.get("CREATED_AT")?,
        updated_at: row.get("UPDATED_AT")?,
    })
}

fn map_window(row: &oracle::Row) -> Result<WindowRecord, oracle::Error> {
    Ok(WindowRecord {
        id: row.get("ID")?,
        location_id: row.get("LOCATION_ID")?,
        weekday: row.get("WEEKDAY")?,
        start_min: row.get("START_MIN")?,
        end_min: row.get("END_MIN")?,
    })
}

fn map_document(row: &oracle::Row) -> Result<DocumentRecord, oracle::Error> {
    Ok(DocumentRecord {
        id: row.get("ID")?,
        client_id: row.get("CLIENT_ID")?,
        kind: row.get("DOC_KIND")?,
        storage_ref: row.get("STORAGE_REF")?,
        status: row.get("STATUS")?,
        note: row.get("NOTE")?,
        created_at: row.get("CREATED_AT")?,
    })
}

fn map_check(row: &oracle::Row) -> Result<CheckRecord, oracle::Error> {
    let passed: i32 = row.get("PASSED")?;
    Ok(CheckRecord {
        id: row.get("ID")?,
        client_id: row.get("CLIENT_ID")?,
        item: row.get("ITEM_CODE")?,
        passed: passed != 0,
        note: row.get("NOTE")?,
        reviewer_id: row.get("REVIEWER_ID")?,
        created_at: row.get("CREATED_AT")?,
    })
}

fn map_event(row: &oracle::Row) -> Result<EventRecord, oracle::Error> {
    Ok(EventRecord {
        id: row.get("ID")?,
        client_id: row.get("CLIENT_ID")?,
        actor_id: row.get("ACTOR_ID")?,
        event_type: row.get("EVENT_TYPE")?,
        from_status: optional_text(row, "FROM_STATUS")?,
        to_status: row.get("TO_STATUS")?,
        payload: optional_text(row, "PAYLOAD")?,
        created_at: row.get("CREATED_AT")?,
    })
}

fn optional_text(row: &oracle::Row, column: &str) -> Result<String, oracle::Error> {
    let value: Option<String> = row.get(column)?;
    Ok(value.unwrap_or_default())
}

fn map_oracle(err: oracle::Error) -> AppError {
    if err.kind() == oracle::ErrorKind::NoDataFound {
        return AppError::NotFound("registro não encontrado".into());
    }
    if let Some(db) = err.db_error() {
        match db.code() {
            1 => return AppError::Conflict("registro já existe".into()),
            2291 => return AppError::NotFound("referência inválida".into()),
            _ => {}
        }
    }
    config::database(err)
}

fn ensure_schema(conn: &Connection) -> Result<(), oracle::Error> {
    for ddl in SCHEMA {
        let statement = ddl.replace('\'', "''");
        let block = format!(
            "BEGIN
                EXECUTE IMMEDIATE '{statement}';
             EXCEPTION
                WHEN OTHERS THEN
                    IF SQLCODE NOT IN (-955, -1408, -2260, -2261, -2275) THEN
                        RAISE;
                    END IF;
             END;"
        );
        conn.execute(&block, &[])?;
    }
    Ok(())
}

const SCHEMA: &[&str] = &[
    "CREATE TABLE WATTLY_USERS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        EMAIL VARCHAR2(180 CHAR) NOT NULL,
        PASSWORD_HASH VARCHAR2(255 CHAR) NOT NULL,
        FULL_NAME VARCHAR2(160 CHAR) NOT NULL,
        PHONE VARCHAR2(20 CHAR) NOT NULL,
        DOC_NUMBER VARCHAR2(20 CHAR) NOT NULL,
        ROLE_NAME VARCHAR2(20 CHAR) NOT NULL,
        STATUS VARCHAR2(20 CHAR) NOT NULL,
        TERMS_VER VARCHAR2(20 CHAR) NOT NULL,
        CONSENT_AT VARCHAR2(40 CHAR) NOT NULL,
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        UPDATED_AT VARCHAR2(40 CHAR) NOT NULL,
        LAST_LOGIN VARCHAR2(40 CHAR),
        CONSTRAINT PK_WATTLY_USERS PRIMARY KEY (ID),
        CONSTRAINT UX_WATTLY_USER_EMAIL UNIQUE (EMAIL)
    )",
    "CREATE TABLE WATTLY_CLIENTS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        OWNER_ID VARCHAR2(36 CHAR) NOT NULL,
        TRADE_NAME VARCHAR2(160 CHAR) NOT NULL,
        LEGAL_NAME VARCHAR2(180 CHAR) NOT NULL,
        PERSON_TYPE VARCHAR2(20 CHAR) NOT NULL,
        TAX_ID VARCHAR2(20 CHAR) NOT NULL,
        PIX_TYPE VARCHAR2(20 CHAR) NOT NULL,
        PIX_KEY VARCHAR2(180 CHAR) NOT NULL,
        BIO VARCHAR2(1000 CHAR),
        STATUS VARCHAR2(24 CHAR) NOT NULL,
        REJECT_REASON VARCHAR2(500 CHAR),
        DECISION_NOTE VARCHAR2(1000 CHAR),
        SUBMITTED_AT VARCHAR2(40 CHAR),
        REVIEWED_AT VARCHAR2(40 CHAR),
        REVIEWER_ID VARCHAR2(36 CHAR),
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        UPDATED_AT VARCHAR2(40 CHAR) NOT NULL,
        CONSTRAINT PK_WATTLY_CLIENTS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_CLIENT_OWN FOREIGN KEY (OWNER_ID) REFERENCES WATTLY_USERS (ID),
        CONSTRAINT FK_WATTLY_CLIENT_REV FOREIGN KEY (REVIEWER_ID) REFERENCES WATTLY_USERS (ID)
    )",
    "CREATE TABLE WATTLY_LOCATIONS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        CLIENT_ID VARCHAR2(36 CHAR) NOT NULL,
        LABEL VARCHAR2(120 CHAR) NOT NULL,
        DESCRIPTION VARCHAR2(1000 CHAR),
        ADDR_LINE VARCHAR2(180 CHAR) NOT NULL,
        ADDR_NUMBER VARCHAR2(20 CHAR) NOT NULL,
        COMPLEMENT VARCHAR2(80 CHAR),
        DISTRICT VARCHAR2(80 CHAR) NOT NULL,
        CITY VARCHAR2(80 CHAR) NOT NULL,
        STATE_CODE VARCHAR2(2 CHAR) NOT NULL,
        POSTAL_CODE VARCHAR2(12 CHAR) NOT NULL,
        COUNTRY_CODE VARCHAR2(2 CHAR) NOT NULL,
        LATITUDE NUMBER(10,7) NOT NULL,
        LONGITUDE NUMBER(10,7) NOT NULL,
        PUB_LAT NUMBER(10,7) NOT NULL,
        PUB_LNG NUMBER(10,7) NOT NULL,
        CONNECTOR VARCHAR2(30 CHAR) NOT NULL,
        POWER_KW NUMBER(6,2) NOT NULL,
        VOLTAGE NUMBER(6) NOT NULL,
        PHASES NUMBER(1) NOT NULL,
        PRICE_MIN_CENTS NUMBER(10) NOT NULL,
        PRICE_HOUR_CENTS NUMBER(10) NOT NULL,
        INSTALL_TYPE VARCHAR2(30 CHAR) NOT NULL,
        HAS_ROOF NUMBER(1) NOT NULL,
        WELL_LIT NUMBER(1) NOT NULL,
        ACCESS_NOTE VARCHAR2(500 CHAR),
        SPOT_CODE VARCHAR2(40 CHAR),
        STATUS VARCHAR2(20 CHAR) NOT NULL,
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        UPDATED_AT VARCHAR2(40 CHAR) NOT NULL,
        CONSTRAINT PK_WATTLY_LOCATIONS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_LOC_CLIENT FOREIGN KEY (CLIENT_ID) REFERENCES WATTLY_CLIENTS (ID)
    )",
    "CREATE TABLE WATTLY_WINDOWS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        LOCATION_ID VARCHAR2(36 CHAR) NOT NULL,
        WEEKDAY NUMBER(1) NOT NULL,
        START_MIN NUMBER(4) NOT NULL,
        END_MIN NUMBER(4) NOT NULL,
        CONSTRAINT PK_WATTLY_WINDOWS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_WIN_LOC FOREIGN KEY (LOCATION_ID) REFERENCES WATTLY_LOCATIONS (ID)
    )",
    "CREATE TABLE WATTLY_DOCUMENTS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        CLIENT_ID VARCHAR2(36 CHAR) NOT NULL,
        DOC_KIND VARCHAR2(40 CHAR) NOT NULL,
        STORAGE_REF VARCHAR2(300 CHAR) NOT NULL,
        STATUS VARCHAR2(20 CHAR) NOT NULL,
        NOTE VARCHAR2(300 CHAR),
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        CONSTRAINT PK_WATTLY_DOCS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_DOC_CLIENT FOREIGN KEY (CLIENT_ID) REFERENCES WATTLY_CLIENTS (ID)
    )",
    "CREATE TABLE WATTLY_CHECKS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        CLIENT_ID VARCHAR2(36 CHAR) NOT NULL,
        ITEM_CODE VARCHAR2(40 CHAR) NOT NULL,
        PASSED NUMBER(1) NOT NULL,
        NOTE VARCHAR2(300 CHAR),
        REVIEWER_ID VARCHAR2(36 CHAR) NOT NULL,
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        CONSTRAINT PK_WATTLY_CHECKS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_CHK_CLIENT FOREIGN KEY (CLIENT_ID) REFERENCES WATTLY_CLIENTS (ID),
        CONSTRAINT FK_WATTLY_CHK_REVIEW FOREIGN KEY (REVIEWER_ID) REFERENCES WATTLY_USERS (ID)
    )",
    "CREATE TABLE WATTLY_CRED_EVENTS (
        ID VARCHAR2(36 CHAR) NOT NULL,
        CLIENT_ID VARCHAR2(36 CHAR) NOT NULL,
        ACTOR_ID VARCHAR2(36 CHAR) NOT NULL,
        EVENT_TYPE VARCHAR2(40 CHAR) NOT NULL,
        FROM_STATUS VARCHAR2(24 CHAR),
        TO_STATUS VARCHAR2(24 CHAR) NOT NULL,
        PAYLOAD VARCHAR2(2000 CHAR),
        CREATED_AT VARCHAR2(40 CHAR) NOT NULL,
        CONSTRAINT PK_WATTLY_EVENTS PRIMARY KEY (ID),
        CONSTRAINT FK_WATTLY_EVT_CLIENT FOREIGN KEY (CLIENT_ID) REFERENCES WATTLY_CLIENTS (ID),
        CONSTRAINT FK_WATTLY_EVT_ACTOR FOREIGN KEY (ACTOR_ID) REFERENCES WATTLY_USERS (ID)
    )",
    "CREATE INDEX IX_WATTLY_CLIENT_OWN ON WATTLY_CLIENTS (OWNER_ID)",
    "CREATE INDEX IX_WATTLY_LOC_CLIENT ON WATTLY_LOCATIONS (CLIENT_ID)",
    "CREATE INDEX IX_WATTLY_WIN_LOC ON WATTLY_WINDOWS (LOCATION_ID)",
    "CREATE INDEX IX_WATTLY_DOC_CLIENT ON WATTLY_DOCUMENTS (CLIENT_ID)",
    "CREATE INDEX IX_WATTLY_EVT_CLIENT ON WATTLY_CRED_EVENTS (CLIENT_ID)",
];

#[cfg(test)]
mod live {
    use super::OracleRepo;
    use crate::config::Config;
    use crate::domain::models::{ClientRecord, LocationRecord, UserRecord, WindowRecord};
    use crate::domain::{now_iso, TERMS_VERSION};
    use crate::repo::Repository;
    use crate::security::hash_password;

    #[test]
    #[ignore = "exige o Oracle da FIAP e as variáveis do arquivo .env"]
    fn oracle_grava_e_le_um_usuario() {
        let _ = dotenvy::from_filename(".env");
        let _ = dotenvy::from_filename("api/.env");
        let config = Config::from_env().expect("variáveis Oracle ausentes");
        let repo = OracleRepo::connect(&config).expect("conexão");
        let banner = repo.ping().expect("ping");
        assert_eq!(banner, "ok");

        let now = now_iso();
        let id = uuid::Uuid::new_v4().to_string();
        let email = format!("roundtrip-{id}@wattly.dev");
        let user = UserRecord {
            id: id.clone(),
            email: email.clone(),
            password_hash: hash_password("Recarga-2026").expect("hash"),
            full_name: "Teste Roundtrip".into(),
            phone: "11987654321".into(),
            document: "52998224725".into(),
            role: "host".into(),
            status: "active".into(),
            terms_version: TERMS_VERSION.into(),
            consent_at: now.clone(),
            created_at: now.clone(),
            updated_at: now.clone(),
            last_login_at: None,
        };
        repo.create_user(user).expect("insert");
        let found = repo
            .find_user_by_email(&email)
            .expect("select")
            .expect("usuário gravado");
        assert_eq!(found.full_name, "Teste Roundtrip");
        assert!(!found.to_public().document_masked.contains("529982247"));

        let client = ClientRecord {
            id: uuid::Uuid::new_v4().to_string(),
            owner_id: id.clone(),
            trade_name: "Ponto Roundtrip".into(),
            legal_name: "Teste Roundtrip".into(),
            person_type: "individual".into(),
            tax_id: "52998224725".into(),
            pix_key_type: "email".into(),
            pix_key: email.clone(),
            bio: String::new(),
            status: "draft".into(),
            reject_reason: None,
            decision_note: None,
            submitted_at: None,
            reviewed_at: None,
            reviewer_id: None,
            created_at: now.clone(),
            updated_at: now.clone(),
        };
        repo.create_client(client.clone()).expect("client");
        let location_id = uuid::Uuid::new_v4().to_string();
        let location = LocationRecord {
            id: location_id.clone(),
            client_id: client.id,
            label: "Ponto Roundtrip".into(),
            description: String::new(),
            address_line: "Rua das Acacias".into(),
            address_number: "120".into(),
            complement: None,
            district: "Vila Mariana".into(),
            city: "Sao Paulo".into(),
            state: "SP".into(),
            postal_code: "04101000".into(),
            country: "BR".into(),
            latitude: -23.561414,
            longitude: -46.655881,
            public_latitude: -23.56,
            public_longitude: -46.66,
            connector: "home_outlet_220v".into(),
            power_kw: 7.4,
            voltage: 220,
            phases: 1,
            price_per_minute_cents: 40,
            price_per_hour_cents: 1800,
            install_type: "home_outlet".into(),
            has_roof: true,
            well_lit: true,
            access_note: Some("Interfone 12".into()),
            spot_code: None,
            status: "pending".into(),
            created_at: now.clone(),
            updated_at: now,
        };
        repo.create_location(
            location,
            vec![WindowRecord {
                id: uuid::Uuid::new_v4().to_string(),
                location_id: location_id.clone(),
                weekday: 1,
                start_min: 480,
                end_min: 1080,
            }],
        )
        .expect("location");
        let (stored, windows) = repo
            .find_location(&location_id)
            .expect("select location")
            .expect("ponto gravado");
        assert!((stored.latitude - -23.561414).abs() < 0.000_001);
        assert_eq!(stored.price_per_minute_cents, 40);
        assert_eq!(windows[0].start_min, 480);
        assert!(stored.has_roof);
        purge(&repo, &id);
    }

    fn purge(repo: &OracleRepo, user_id: &str) {
        let _ = repo.with_conn(|conn| {
            conn.execute(
                "DELETE FROM WATTLY_CRED_EVENTS WHERE ACTOR_ID = :1
                 OR CLIENT_ID IN (SELECT ID FROM WATTLY_CLIENTS WHERE OWNER_ID = :2)",
                &[&user_id, &user_id],
            )?;
            conn.execute(
                "DELETE FROM WATTLY_CHECKS WHERE REVIEWER_ID = :1
                 OR CLIENT_ID IN (SELECT ID FROM WATTLY_CLIENTS WHERE OWNER_ID = :2)",
                &[&user_id, &user_id],
            )?;
            conn.execute(
                "DELETE FROM WATTLY_DOCUMENTS WHERE CLIENT_ID IN (SELECT ID FROM WATTLY_CLIENTS WHERE OWNER_ID = :1)",
                &[&user_id],
            )?;
            conn.execute(
                "DELETE FROM WATTLY_WINDOWS WHERE LOCATION_ID IN (
                    SELECT L.ID FROM WATTLY_LOCATIONS L
                    JOIN WATTLY_CLIENTS C ON C.ID = L.CLIENT_ID
                    WHERE C.OWNER_ID = :1
                 )",
                &[&user_id],
            )?;
            conn.execute(
                "DELETE FROM WATTLY_LOCATIONS WHERE CLIENT_ID IN (SELECT ID FROM WATTLY_CLIENTS WHERE OWNER_ID = :1)",
                &[&user_id],
            )?;
            conn.execute("DELETE FROM WATTLY_CLIENTS WHERE OWNER_ID = :1", &[&user_id])?;
            conn.execute("DELETE FROM WATTLY_USERS WHERE ID = :1", &[&user_id])?;
            Ok(())
        });
    }
}
