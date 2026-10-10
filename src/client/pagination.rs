use std::{collections::BTreeMap, future::Future};

use crate::{
    error::BojError,
    model::{CodeResponse, CodeSeries, DataPoint, LayerResponse, LayerSeries},
    query::{CodeQuery, LayerQuery},
};

use super::BojClient;

impl BojClient {
    /// Fetches all remaining code pages and merges their series and data points.
    ///
    /// Starts at the query's `startPosition` (or 1), preserving all filters.
    /// Raw page bodies are omitted. Returns only after the continuation cursor
    /// disappears. The deadline applies to each page, not the entire history.
    ///
    /// # Errors
    /// Returns request/decode errors or a pagination error for non-advancing
    /// cursors, conflicting data, or an unfinished result at the page ceiling.
    pub async fn get_data_code_all(&self, query: &CodeQuery) -> Result<Vec<CodeSeries>, BojError> {
        collect_pages(query.clone(), self.max_pages, |query| async move {
            self.get_data_code(&query).await
        })
        .await
    }

    /// Fetches all remaining layer pages and merges their series and data points.
    ///
    /// Has the same completion guarantees and per-page deadline as
    /// [`Self::get_data_code_all`].
    ///
    /// # Errors
    /// Returns request/decode errors or a pagination error for incomplete or
    /// inconsistent results. No partial data is returned on failure.
    pub async fn get_data_layer_all(
        &self,
        query: &LayerQuery,
    ) -> Result<Vec<LayerSeries>, BojError> {
        collect_pages(query.clone(), self.max_pages, |query| async move {
            self.get_data_layer(&query).await
        })
        .await
    }
}

trait PageQuery: Sized {
    fn position(&self) -> u32;
    fn at_position(self, position: u32) -> Result<Self, BojError>;
}

macro_rules! page_query {
    ($query:ty) => {
        impl PageQuery for $query {
            fn position(&self) -> u32 {
                self.query_pairs()
                    .iter()
                    .find(|(key, _)| key == "startPosition")
                    .and_then(|(_, value)| value.parse().ok())
                    .unwrap_or(1)
            }
            fn at_position(self, position: u32) -> Result<Self, BojError> {
                self.with_start_position(position)
            }
        }
    };
}
page_query!(CodeQuery);
page_query!(LayerQuery);

trait Page {
    type Series: Series;
    fn into_parts(self) -> (Option<u32>, Vec<Self::Series>);
}

macro_rules! page {
    ($page:ty, $series:ty) => {
        impl Page for $page {
            type Series = $series;
            fn into_parts(self) -> (Option<u32>, Vec<Self::Series>) {
                (self.next_position, self.series)
            }
        }
    };
}
page!(CodeResponse, CodeSeries);
page!(LayerResponse, LayerSeries);

trait Series {
    fn code(&self) -> &str;
    fn points_mut(&mut self) -> &mut Vec<DataPoint>;
}

macro_rules! series {
    ($series:ty) => {
        impl Series for $series {
            fn code(&self) -> &str {
                &self.series_code
            }
            fn points_mut(&mut self) -> &mut Vec<DataPoint> {
                &mut self.points
            }
        }
    };
}
series!(CodeSeries);
series!(LayerSeries);

async fn collect_pages<Q, P, F, Fut>(
    mut query: Q,
    max_pages: usize,
    fetch: F,
) -> Result<Vec<P::Series>, BojError>
where
    Q: PageQuery + Clone,
    P: Page,
    F: Fn(Q) -> Fut,
    Fut: Future<Output = Result<P, BojError>>,
{
    let mut result: Vec<P::Series> = Vec::new();
    let mut indices = BTreeMap::<String, usize>::new();
    for page in 0..max_pages {
        let current = query.position();
        let (next, rows) = fetch(query.clone()).await?.into_parts();
        for row in rows {
            if let Some(&index) = indices.get(row.code()) {
                merge_points(&mut result[index], row)?;
            } else {
                indices.insert(row.code().to_owned(), result.len());
                let mut row = row;
                let points = std::mem::take(row.points_mut());
                append_points(&mut row, points)?;
                result.push(row);
            }
        }
        let Some(next) = next else {
            return Ok(result);
        };
        if next <= current {
            return Err(BojError::PaginationError(format!(
                "cursor did not advance: {current} -> {next}"
            )));
        }
        if page + 1 == max_pages {
            return Err(BojError::PaginationError(format!(
                "page limit {max_pages} reached with continuation {next}"
            )));
        }
        query = query.at_position(next)?;
    }
    Err(BojError::PaginationError(
        "page limit must be positive".into(),
    ))
}

fn merge_points<S: Series>(target: &mut S, mut source: S) -> Result<(), BojError> {
    append_points(target, std::mem::take(source.points_mut()))
}

fn append_points<S: Series>(target: &mut S, incoming: Vec<DataPoint>) -> Result<(), BojError> {
    let mut points: BTreeMap<String, Option<String>> = std::mem::take(target.points_mut())
        .into_iter()
        .map(|point| (point.survey_date, point.value))
        .collect();
    for point in incoming {
        if let Some(value) = points.get(&point.survey_date)
            && value != &point.value
        {
            return Err(BojError::PaginationError(format!(
                "conflicting values for {} at {}",
                target.code(),
                point.survey_date
            )));
        }
        points.insert(point.survey_date, point.value);
    }
    *target.points_mut() = points
        .into_iter()
        .map(|(survey_date, value)| DataPoint { survey_date, value })
        .collect();
    Ok(())
}
