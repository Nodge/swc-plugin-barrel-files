Правила оптимизации:

- заранее знаем алиасы на другие слои
- оптимизируем только импорты по абсолютным путям + энтрипоинты пакетов
- в конфиг плагина задаем список паттернов, которые надо оптимизировать
- внутри индексного файла не может быть никакой другой код, кроме ре-экспортов. Если есть, то кидаем ошибку парсинга

как работает резолвинг:

- в файле ищем все импорты
- в импорте находим паттерн
- по паттерну понимаем целевой файл (тут нужен резолвинг)
- читаем и парсим целевой файл (тут нужен кеш, по времени модификации?)
- в файле должны быть только ре-экспорты
- находим импортируемый символ, по нему определяем целевой файл и имя символа в нем
- переписываем исходный импорт на прямой путь до файла

сложные кейсы:

- переименовывание идентификаторов в ре-экспортах (сложно в сочетании с jest.mock)
- export \* (у нас запрещен, так что ок)
- сайд-эффекты в ре-экспортируемых файлах (нужно сверятся с pkg.sideEffects)

вопросы:

- можно ли считать алиасы автоматически из package.json?
- как указывать пакеты, которые нужно поомтимизировать?

```
// config:
{
  packages: [
     '@direct-frontend/stdlib',
     '@direct-frontend/components',
  ],
  rules: [
    {
      pattern: '#entities/*',
      paths: ['src/entities/*/index.ts']
    },
    {
      pattern: '#entities/*/testing',
      paths: ['src/entities/*/testing.ts']
    },
    {
      pattern: '#entities/*/inline',
      paths: ['src/entities/*/inline.ts']
    },
    {
      pattern: '#shared/api/*',
      paths: ['src/shared/api/*/index.ts']
    },
    {
      pattern: '#shared/components/*',
      paths: ['src/shared/components/*/index.ts']
    },
    {
      pattern: '#shared/lib/*',
      paths: ['src/shared/lib/*/index.ts']
    },
  ]
}

// index:
export { Button } from './components/Button';
export { select } from './model/selector';

// testing:
export { mock } from './api/mocks/test';

// before:
import { Button, select } from '#features/test';
import { mock } from '#features/test/testing';

// after:
import { Button } from '../features/test/components/Button';
import { select } from '../features/test/model/selector';
import { mock } from '../features/test/api/mocks/test';
```

- как работают паттерны
- обработка export \* from '...'
- как работают вложенные barrel файлы
- что делать, если в экспортах не найден символ
- кеш файловой системы должен инвалидироваться по времени, время настраивается через конфиг
- юнит тесты пишем на rust
- интеграционные тесты пишем на typescript внутри директории tests
