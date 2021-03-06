# TagChat

## Autorzy
- Szymon Łukasik (gr 9, @SzymonLukasik na githubie)
- Michał Molas (gr 9, @michal-molas na githubie)

## Opis
Od zawsze chcieliśmy napisać aplikację do chatu.

## Inspiracje
Z grubsza będziemy wzorować się na tym tutorialu https://www.youtube.com/watch?v=Iapc-qGTEBQ.
Możliwe, że też na tym w szczegolności gdybyśmy chcieli P2P https://github.com/lemunozm/termchat.

Pracując nad pierwszą częścią inspirowaliśmy się przykładami:
- https://github.com/tokio-rs/tokio/tree/master/examples  (przykładowe programy wykorzystujące tokio, pochodzi stąd serwer, którego używamy)
- https://github.com/emilk/eframe_template  (przykład wykorzystania frameworku eframe)
- https://github.com/emilk/egui/tree/master/egui_demo_lib/src/demo (widgety z dema egui)
- https://tokio.rs/tokio/topics/bridging (ostatni przykład ostatecznie wykorzystaliśmy w celu integracji GUI z kodem asynchronicznym)

## Funkcjonalność
- Możliwość tagowania fragmentów konwersacji, to mogłoby umożliwiać:
  - określanie tematyki fragmentu konwersacji
  - grupowanie fragmentów konwersacji po tagach
  - zwracanie wyników wyszukiwania pogrupowanych według tagów
- Użycie współbieżności
- Możliwość połączenia video i udostępniania ekranu

## Propozycja podziału na części
W pierwszej części stworzymy prosty server i GUI, jeśli zdążymy z możliwośćią wyszukiwania. 
A w kolejnej dodamy współbieżność i połączenia video.

## Część Pierwsza
W pierwszej części największą trudnością okazało się zintegrowanie GUI z kodem asynchronicznym.
Do obsługi połączenia TCP wykorzystujemy bibliotekę tokio.
Z początku, w celu połączenia egui z tokio inspirowaliśmy się sugestią twórcy biblioteki egui:
https://github.com/emilk/egui/discussions/634#discussioncomment-2035285
Próbowaliśmy użyć biblioteki poll-promise, aby w odpowiednich miejscach funkcji eframe::App::update, móc przesyłać lub pobierać informacje z serwera.
Nie udało nam się jednak poprawnie zaimplementować tego pomysłu - każde podejście kończyło się błędem kompilacji.
Ostatecznie zdecydowaliśmy się na zmianę architektury aplikacji - w konstruktorze TagChatApp tworzymy osobny wątek, który odpowiada za komunikację z serwerem i wykonuje działania asynchroniczne. Wątkek GUI i obsługujący komunikacje z serwerem komunikują się ze sobą za pomocą channeli.

Klient prowadzi konwersację ze wszystkimi innymi klientami podłączonymi do serwera.
Klient umożliwia wyszukiwanie słów kluczowych w wiadomościach.
Nie zaimplementowaliśmy możliwości tagowania fragmentów konwersacji oraz łączenia video.

W kolejnej części moglibyśmy na przykład dodać możliwość tagowania fragmentów konwersacji, rozwinąć aplikację o wspieranie kont, obsługę różnych konwersacji, łączenie video i zwiększyć wykorzystanie biblioteki tokio.

## Biblioteki
- tokio
- egui
- eframe (framework do egui)

## Użytowanie
W poniższych poleceniach addr jest adresem IPv4 lub IPv6.

W celu uruchomienia serwera na porcie port: \
<code>cargo run --bin server -- -p port </code>

W celu uruchomienia klienta z nazwą użytkownika user_name, jeśli server działa pod adresem addr na porcie port:  \
<code> cargo run --bin client -- -s addr:port -n user_name</code> 

Przykład 1 - Serwer działa lokalnie na porcie 1234

W jednym terminalu: \
<code>cargo run --bin server -- -p 1234 </code>

W dwóch terminalach: \
<code> cargo run --bin client -- -s localhost:1234 user_name</code> 


Przykład 2 - Serwer działa na students na porcie 1234

W terminalu na students: \
<code>cargo run --bin server -- -p 1234 </code>

W dwóch terminalach lokalnie: \
<code> cargo run --bin client -- s students.mimuw.edu.pl:1234 -n user_name</code>



