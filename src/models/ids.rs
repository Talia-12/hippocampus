use std::{convert::Infallible, fmt, str::FromStr};

use diesel::{
	deserialize::{FromSql, FromSqlRow},
	expression::AsExpression,
	serialize::{self, IsNull, Output, ToSql},
	sql_types::Text,
	sqlite::{Sqlite, SqliteValue},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

///
#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Hash,
	Serialize,
	Deserialize,
	AsExpression,
	FromSqlRow,
)]
#[diesel(sql_type = Text)]
pub struct TagId(pub String);

impl TagId {
	pub fn new() -> Self {
		Self(format!("tag-{}", Uuid::new_v4()))
	}
}

impl FromSql<Text, Sqlite> for TagId {
	fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
		let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
		Ok(TagId(text))
	}
}

impl ToSql<Text, Sqlite> for TagId {
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
		out.set_value(self.0.clone());
		Ok(IsNull::No)
	}
}

impl FromStr for TagId {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(TagId(s.to_owned()))
	}
}

impl fmt::Display for TagId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

///
#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	Hash,
	PartialOrd,
	Ord,
	Serialize,
	Deserialize,
	AsExpression,
	FromSqlRow,
)]
#[diesel(sql_type = Text)]
pub struct CardId(pub String);

impl CardId {
	pub fn new() -> Self {
		Self(format!("card-{}", Uuid::new_v4()))
	}
}

impl FromSql<Text, Sqlite> for CardId {
	fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
		let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
		Ok(CardId(text))
	}
}

impl ToSql<Text, Sqlite> for CardId {
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
		out.set_value(self.0.clone());
		Ok(IsNull::No)
	}
}

impl FromStr for CardId {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(CardId(s.to_owned()))
	}
}

impl fmt::Display for CardId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

///
#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	Hash,
	PartialOrd,
	Ord,
	Serialize,
	Deserialize,
	AsExpression,
	FromSqlRow,
)]
#[diesel(sql_type = Text)]
pub struct ItemId(pub String);

impl ItemId {
	pub fn new() -> Self {
		Self(format!("item-{}", Uuid::new_v4()))
	}
}

impl FromSql<Text, Sqlite> for ItemId {
	fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
		let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
		Ok(ItemId(text))
	}
}

impl ToSql<Text, Sqlite> for ItemId {
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
		out.set_value(self.0.clone());
		Ok(IsNull::No)
	}
}

impl FromStr for ItemId {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(ItemId(s.to_owned()))
	}
}

impl fmt::Display for ItemId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

///
#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	PartialOrd,
	Ord,
	Hash,
	Serialize,
	Deserialize,
	AsExpression,
	FromSqlRow,
)]
#[diesel(sql_type = Text)]
pub struct ReviewId(pub String);

impl ReviewId {
	pub fn new() -> Self {
		Self(format!("review-{}", Uuid::new_v4()))
	}
}

impl FromSql<Text, Sqlite> for ReviewId {
	fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
		let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
		Ok(ReviewId(text))
	}
}

impl ToSql<Text, Sqlite> for ReviewId {
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
		out.set_value(self.0.clone());
		Ok(IsNull::No)
	}
}

impl FromStr for ReviewId {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(ReviewId(s.to_owned()))
	}
}

impl fmt::Display for ReviewId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}

///
#[derive(
	Debug,
	Clone,
	PartialEq,
	Eq,
	Hash,
	PartialOrd,
	Ord,
	Serialize,
	Deserialize,
	AsExpression,
	FromSqlRow,
)]
#[diesel(sql_type = Text)]
pub struct ItemTypeId(pub String);

impl ItemTypeId {
	pub fn new() -> Self {
		Self(format!("item-type-{}", Uuid::new_v4()))
	}
}

impl FromSql<Text, Sqlite> for ItemTypeId {
	fn from_sql(value: SqliteValue<'_, '_, '_>) -> diesel::deserialize::Result<Self> {
		let text = <String as FromSql<Text, Sqlite>>::from_sql(value)?;
		Ok(ItemTypeId(text))
	}
}

impl ToSql<Text, Sqlite> for ItemTypeId {
	fn to_sql<'b>(&'b self, out: &mut Output<'b, '_, Sqlite>) -> serialize::Result {
		out.set_value(self.0.clone());
		Ok(IsNull::No)
	}
}

impl FromStr for ItemTypeId {
	type Err = Infallible;

	fn from_str(s: &str) -> Result<Self, Self::Err> {
		Ok(ItemTypeId(s.to_owned()))
	}
}

impl fmt::Display for ItemTypeId {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		write!(f, "{}", self.0)
	}
}
