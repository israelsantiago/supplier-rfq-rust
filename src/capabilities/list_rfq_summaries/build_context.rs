use super::process_request::{ListRfqSummariesError, ListRfqSummariesRequest};

/// Contexto normalizado da query; contém apenas valores já validados.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct NormalizedListRequest {
    pub status: Option<String>,
    pub limit: i64,
    pub offset: i64,
}

/// Decisão pura de validação/normalização dos parâmetros recebidos.
pub(super) fn build_context(
    request: ListRfqSummariesRequest,
) -> Result<NormalizedListRequest, ListRfqSummariesError> {
    let limit = request.limit.unwrap_or(50);
    let offset = request.offset.unwrap_or(0);

    if !(1..=100).contains(&limit) {
        return Err(ListRfqSummariesError::InvalidLimit);
    }

    if offset < 0 {
        return Err(ListRfqSummariesError::InvalidOffset);
    }

    let status = match request.status.as_deref() {
        None => None,
        Some("open") => Some("open".to_owned()),
        Some("closed") => Some("closed".to_owned()),
        Some(_) => return Err(ListRfqSummariesError::InvalidStatus),
    };

    Ok(NormalizedListRequest {
        status,
        limit,
        offset,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn request(
        status: Option<&str>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> ListRfqSummariesRequest {
        ListRfqSummariesRequest {
            status: status.map(str::to_owned),
            limit,
            offset,
        }
    }

    #[test]
    fn defaults_are_limit_50_offset_0_without_status_filter() {
        assert_eq!(
            build_context(request(None, None, None)).unwrap(),
            NormalizedListRequest {
                status: None,
                limit: 50,
                offset: 0,
            }
        );
    }

    #[test]
    fn accepts_only_open_and_closed_statuses() {
        assert_eq!(
            build_context(request(Some("open"), None, None))
                .unwrap()
                .status
                .as_deref(),
            Some("open")
        );
        assert_eq!(
            build_context(request(Some("closed"), None, None))
                .unwrap()
                .status
                .as_deref(),
            Some("closed")
        );
        assert!(matches!(
            build_context(request(Some("cancelled"), None, None)),
            Err(ListRfqSummariesError::InvalidStatus)
        ));
    }

    #[test]
    fn rejects_limits_outside_one_to_one_hundred() {
        for limit in [0, -1, 101] {
            assert!(matches!(
                build_context(request(None, Some(limit), None)),
                Err(ListRfqSummariesError::InvalidLimit)
            ));
        }
    }

    #[test]
    fn rejects_negative_offset() {
        assert!(matches!(
            build_context(request(None, None, Some(-1))),
            Err(ListRfqSummariesError::InvalidOffset)
        ));
    }

    #[test]
    fn accepts_inclusive_pagination_boundaries() {
        assert_eq!(
            build_context(request(None, Some(1), Some(0)))
                .unwrap()
                .limit,
            1
        );
        assert_eq!(
            build_context(request(None, Some(100), Some(0)))
                .unwrap()
                .limit,
            100
        );
    }

    proptest! {
        #[test]
        fn property_valid_pagination_bounds_are_preserved(
            limit in 1_i64..=100,
            offset in 0_i64..=1_000_000,
        ) {
            let normalized = build_context(request(None, Some(limit), Some(offset))).unwrap();
            prop_assert_eq!(normalized.limit, limit);
            prop_assert_eq!(normalized.offset, offset);
            prop_assert_eq!(normalized.status, None);
        }
    }
}
