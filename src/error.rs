use thiserror::Error;

#[derive(Error, Debug)]
pub enum WifiMonitorError {
    #[error("Ошибка базы данных: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Сетевой адаптер не найден: {0}")]
    AdapterNotFound(String),

    #[error("Не удалось создать канал связи: {0}")]
    ChannelCreation(String),

    #[error("Ошибка парсинга пакетов: {0}")]
    PacketParsing(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Ошибка конфигурации: {0}")]
    Config(String),
}

pub type Result<T> = std::result::Result<T, WifiMonitorError>;